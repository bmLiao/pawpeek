use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 工作中 / 等待確認 / 工具出錯超過這段時間沒有任何事件，視為對話已失聯（例如終端機被直接關掉、
/// 沒送出 SessionEnd），自動回到閒置，避免寵物永遠卡在該狀態。只重置狀態、不移除對話。
pub const STALE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightState {
    Idle = 1,
    Running = 2,
    ToolError = 3,
    ErrorFinal = 4,
    NeedConfirm = 5,
}

impl LightState {
    pub fn priority(&self) -> u8 {
        *self as u8
    }

    pub fn should_blink(&self) -> bool {
        matches!(
            self,
            LightState::Running | LightState::ToolError | LightState::NeedConfirm
        )
    }

    pub fn blink_interval_ms(&self) -> u64 {
        match self {
            LightState::Running => 900,
            LightState::NeedConfirm => 380,
            LightState::ToolError => 280,
            _ => 500,
        }
    }
}

#[derive(Debug)]
struct Session {
    state: LightState,
    last_event: Instant,
    /// 執行這個對話 hook 的 agent 程序：(PID, 啟動時間)。agent 結束就把對話當作結束（見 agent.rs）
    agent: Option<(u32, Option<u64>)>,
    /// 工作資料夾的名稱（hook 的 `cwd` 最後一層）
    folder: Option<String>,
}

impl Session {
    fn new() -> Self {
        Session { state: LightState::Idle, last_event: Instant::now(), agent: None, folder: None }
    }
}

/// 從工作資料夾路徑取出最後一層名稱（`C:\Users\me\pawpeek` → `pawpeek`）；取不到回傳 None
pub fn folder_name(cwd: &str) -> Option<String> {
    let name = cwd.trim().trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()?.trim();
    // 只剩磁碟機代號（`C:`）不算資料夾名稱
    if name.is_empty() || name.ends_with(':') { None } else { Some(name.to_string()) }
}

#[derive(Debug)]
pub struct StateMachine {
    sessions: HashMap<String, Session>,
    session_counter: u32,
    current_state: LightState,
    /// 造成目前狀態的對話的資料夾名稱（合併狀態改變時更新；分不出是哪個對話時為 None）
    current_folder: Option<String>,
    /// 對話已結束、但程式（agent 程序）還開著的 agent：(PID, 啟動時間)。
    /// 寵物只在程式關掉才離開，所以這些 agent 還在時就算沒有對話也留著（見 `end_dead_agents`）
    lingering: Vec<(u32, Option<u64>)>,
    should_exit: bool,
}

impl StateMachine {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            session_counter: 0,
            current_state: LightState::Idle,
            current_folder: None,
            lingering: Vec::new(),
            should_exit: false,
        }
    }

    pub fn current_state(&self) -> LightState {
        self.current_state
    }

    /// 造成目前狀態的對話的資料夾名稱（見 `update_current_state`）
    pub fn current_folder(&self) -> Option<String> {
        self.current_folder.clone()
    }

    pub fn session_count(&self) -> u32 {
        self.session_counter
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }

    pub fn handle_event(&mut self, session_id: &str, event: &str) {
        self.handle_event_in(session_id, event, None);
    }

    /// 同 `handle_event`，並記下這個對話的工作資料夾（hook 的 `cwd`）
    pub fn handle_event_in(&mut self, session_id: &str, event: &str, cwd: Option<&str>) {
        match event {
            "session_start" => {
                let is_new = !self.sessions.contains_key(session_id);
                if is_new {
                    // 真正的新 session：计数 +1，初始化为 Idle
                    self.session_counter += 1;
                    self.sessions.insert(session_id.to_string(), Session::new());
                }
                // is_new=false：running 比 session_start 先到，
                // session 已存在且状态正确，不覆盖。
                // compact/clear 场景下 Stop 事件会负责重置为 Idle。
                if self.session_counter > 0 {
                    self.should_exit = false;
                }
            }

            "session_end" => {
                let removed = self.sessions.remove(session_id);
                if removed.is_some() && self.session_counter > 0 {
                    self.session_counter -= 1;
                }
                // 對話結束不代表程式關掉（例如 VS Code 開新聊天）：記下它的 agent，程式還開著就不離開
                if let Some(agent) = removed.and_then(|s| s.agent) {
                    if !self.lingering.iter().any(|a| a.0 == agent.0) { self.lingering.push(agent); }
                }
                self.update_exit();
            }

            "stop" => {
                if let Some(session) = self.sessions.get_mut(session_id) {
                    session.last_event = Instant::now();
                    if !matches!(session.state, LightState::ErrorFinal) {
                        session.state = LightState::Idle;
                    }
                }
            }

            "running"      => self.set_session_state(session_id, LightState::Running),
            "need_confirm" => self.set_session_state(session_id, LightState::NeedConfirm),
            "tool_error"   => self.set_session_state(session_id, LightState::ToolError),
            "error_final"  => self.set_session_state(session_id, LightState::ErrorFinal),
            "idle"         => self.set_session_state(session_id, LightState::Idle),

            _ => {}
        }

        if let (Some(s), Some(name)) = (self.sessions.get_mut(session_id), cwd.and_then(folder_name)) {
            s.folder = Some(name);
        }
        self.update_current_state(Some(session_id));
    }

    fn set_session_state(&mut self, session_id: &str, state: LightState) {
        // 若 session 不存在（事件乱序：running 比 session_start 先到），
        // 隐式创建并同步增加 counter，保证 session_end 时计数对称。
        let is_new = !self.sessions.contains_key(session_id);
        if is_new {
            self.session_counter += 1;
            self.should_exit = false;
        }
        let session = self.sessions
            .entry(session_id.to_string())
            .or_insert_with(Session::new);
        session.state = state;
        session.last_event = Instant::now();
    }

    /// 這個對話是否已記錄為 `pid` 這個 agent
    pub fn has_agent(&self, session_id: &str, pid: u32) -> bool {
        self.sessions.get(session_id).and_then(|s| s.agent).map(|(p, _)| p) == Some(pid)
    }

    /// 記下對話的 agent 程序（對話不存在就不記）
    pub fn set_agent(&mut self, session_id: &str, pid: u32, start: Option<u64>) {
        if let Some(s) = self.sessions.get_mut(session_id) {
            s.agent = Some((pid, start));
        }
    }

    /// agent 程序已結束的對話一律當作收到 SessionEnd（關掉 VS Code / 終端機時常常不送），
    /// 並清掉已關掉的程式；沒有對話、也沒有還開著的程式時才離開。
    /// `alive(pid, 啟動時間)` 判斷程序是否還在。回傳結束了幾個對話。
    pub fn end_dead_agents(&mut self, alive: impl Fn(u32, Option<u64>) -> bool) -> usize {
        let dead: Vec<String> = self.sessions.iter()
            .filter(|(_, s)| matches!(s.agent, Some((pid, start)) if !alive(pid, start)))
            .map(|(id, _)| id.clone())
            .collect();
        for id in &dead {
            self.handle_event(id, "session_end");
        }
        let before = self.lingering.len();
        self.lingering.retain(|&(pid, start)| alive(pid, start));
        if self.lingering.len() != before { self.update_exit(); }
        dead.len()
    }

    /// 沒有對話、也沒有還開著的程式（agent）時離開。
    /// 沒有 agent 資訊的對話（非 Windows、找不到 agent）結束時，維持「對話數變 0 就離開」。
    fn update_exit(&mut self) {
        self.should_exit = self.session_counter == 0 && self.lingering.is_empty();
    }

    /// 把超過 `timeout` 沒有事件、仍停在工作中 / 等待確認 / 工具出錯的對話重置為閒置。
    /// 嚴重錯誤刻意保留（要等使用者下一次操作才清掉），閒置對話也不移除。
    /// 回傳是否有對話被重置。
    pub fn expire_stale(&mut self, now: Instant, timeout: Duration) -> bool {
        let mut changed = false;
        for session in self.sessions.values_mut() {
            let active = matches!(
                session.state,
                LightState::Running | LightState::NeedConfirm | LightState::ToolError
            );
            if active && now.saturating_duration_since(session.last_event) >= timeout {
                session.state = LightState::Idle;
                changed = true;
            }
        }
        if changed {
            self.update_current_state(None);
        }
        changed
    }

    /// 重算合併狀態（取最高優先）。狀態改變時記下是哪個對話造成的資料夾名稱：
    /// 觸發事件的對話正處於新狀態就用它；否則處於新狀態的對話只有一個就用那個；都分不出來就不顯示。
    fn update_current_state(&mut self, trigger: Option<&str>) {
        let new = self
            .sessions
            .values()
            .map(|s| s.state)
            .max_by_key(|s| s.priority())
            .unwrap_or(LightState::Idle);
        if new != self.current_state {
            let trig = trigger.and_then(|id| self.sessions.get(id)).filter(|s| s.state == new);
            let holder = trig.or_else(|| {
                let mut it = self.sessions.values().filter(|s| s.state == new);
                match (it.next(), it.next()) { (Some(only), None) => Some(only), _ => None }
            });
            self.current_folder = holder.and_then(|s| s.folder.clone());
        }
        self.current_state = new;
    }

    #[cfg(test)]
    pub fn reset_all(&mut self) {
        self.sessions.clear();
        self.session_counter = 0;
        self.should_exit = false;
        self.current_state = LightState::Idle;
        self.current_folder = None;
        self.lingering.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_running_before_session_start() {
        // running 先到，session_start 后到
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "running");
        assert_eq!(sm.session_count(), 1);
        assert_eq!(sm.current_state(), LightState::Running); // 闪烁绿灯

        sm.handle_event("s1", "session_start"); // 后到，不覆盖状态
        assert_eq!(sm.session_count(), 1);      // counter 不重复 +1
        assert_eq!(sm.current_state(), LightState::Running); // 仍然闪烁！

        sm.handle_event("s1", "session_end");
        assert_eq!(sm.session_count(), 0);
        assert!(sm.should_exit());
    }

    #[test]
    fn test_session_start_before_running() {
        // session_start 先到（正常顺序）
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "session_start");
        assert_eq!(sm.session_count(), 1);
        assert_eq!(sm.current_state(), LightState::Idle);

        sm.handle_event("s1", "running");
        assert_eq!(sm.current_state(), LightState::Running); // 闪烁绿灯

        sm.handle_event("s1", "stop");
        assert_eq!(sm.current_state(), LightState::Idle);

        sm.handle_event("s1", "session_end");
        assert_eq!(sm.session_count(), 0);
        assert!(sm.should_exit());
    }

    #[test]
    fn dead_agent_ends_its_sessions() {
        let mut sm = StateMachine::new();
        sm.handle_event("vscode-a", "running");
        sm.handle_event("vscode-b", "session_start");
        sm.handle_event("claude", "session_start");
        sm.set_agent("vscode-a", 10, Some(1));
        sm.set_agent("vscode-b", 10, Some(1));
        sm.set_agent("claude", 20, Some(2));
        assert!(sm.has_agent("vscode-a", 10));
        // VS Code（pid 10）關掉：它的兩個對話結束，Claude 的留著
        assert_eq!(sm.end_dead_agents(|pid, _| pid != 10), 2);
        assert_eq!(sm.session_count(), 1);
        assert!(!sm.should_exit());
        assert_eq!(sm.current_state(), LightState::Idle);
        // Claude 也關掉 → 全部結束，寵物離開
        assert_eq!(sm.end_dead_agents(|_, _| false), 1);
        assert_eq!(sm.session_count(), 0);
        assert!(sm.should_exit());
    }

    #[test]
    fn session_end_keeps_pet_while_program_runs() {
        // VS Code 開新聊天：舊聊天送 SessionEnd，但 VS Code（agent）還開著 → 不離開
        let mut sm = StateMachine::new();
        sm.handle_event("chat1", "session_start");
        sm.set_agent("chat1", 10, Some(1));
        sm.handle_event("chat1", "session_end");
        assert_eq!(sm.session_count(), 0);
        assert!(!sm.should_exit());
        // 程式還在：檢查後仍不離開
        assert_eq!(sm.end_dead_agents(|_, _| true), 0);
        assert!(!sm.should_exit());
        // 新聊天加入、又結束：仍不離開
        sm.handle_event("chat2", "session_start");
        sm.set_agent("chat2", 10, Some(1));
        sm.handle_event("chat2", "session_end");
        assert!(!sm.should_exit());
        // 關掉 VS Code → 離開
        sm.end_dead_agents(|_, _| false);
        assert!(sm.should_exit());
    }

    #[test]
    fn leaves_only_when_all_programs_closed() {
        let mut sm = StateMachine::new();
        sm.handle_event("vscode", "session_start");
        sm.set_agent("vscode", 10, Some(1));
        sm.handle_event("cli", "session_start");
        sm.set_agent("cli", 20, Some(2));
        sm.handle_event("vscode", "session_end");
        sm.handle_event("cli", "session_end");
        assert!(!sm.should_exit());
        // 只關掉 CLI：VS Code 還開著
        sm.end_dead_agents(|pid, _| pid == 10);
        assert!(!sm.should_exit());
        sm.end_dead_agents(|_, _| false);
        assert!(sm.should_exit());
    }

    #[test]
    fn sessions_without_agent_are_not_ended() {
        // 舊版 hook 或非 Windows：沒有 agent 資訊的對話只靠 SessionEnd / 逾時
        let mut sm = StateMachine::new();
        sm.handle_event("s", "session_start");
        assert_eq!(sm.end_dead_agents(|_, _| false), 0);
        assert_eq!(sm.session_count(), 1);
    }

    #[test]
    fn test_session_counter_no_leak_on_compact() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "session_start");
        assert_eq!(sm.session_count(), 1);
        sm.handle_event("s1", "session_start"); // compact/clear 重复触发
        assert_eq!(sm.session_count(), 1);      // 不累加
        sm.handle_event("s1", "session_end");
        assert_eq!(sm.session_count(), 0);
        assert!(sm.should_exit());
    }

    #[test]
    fn test_multi_session_priority() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "session_start");
        sm.handle_event("s2", "session_start");
        sm.handle_event("s1", "running");
        sm.handle_event("s2", "need_confirm");
        assert_eq!(sm.current_state(), LightState::NeedConfirm);
        sm.handle_event("s2", "stop");
        assert_eq!(sm.current_state(), LightState::Running);
    }

    #[test]
    fn test_error_final_not_overwritten_by_stop() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "session_start");
        sm.handle_event("s1", "error_final");
        sm.handle_event("s1", "stop");
        assert_eq!(sm.current_state(), LightState::ErrorFinal);
    }

    #[test]
    fn test_expire_stale_resets_active_sessions_only() {
        let mut sm = StateMachine::new();
        sm.handle_event("run", "running");
        sm.handle_event("ask", "need_confirm");
        sm.handle_event("err", "error_final");
        sm.handle_event("idle", "session_start");
        let later = Instant::now() + STALE_TIMEOUT;

        assert!(sm.expire_stale(later, STALE_TIMEOUT));
        // 嚴重錯誤保留，其餘回到閒置
        assert_eq!(sm.current_state(), LightState::ErrorFinal);
        sm.handle_event("err", "running");
        sm.handle_event("err", "stop");
        assert_eq!(sm.current_state(), LightState::Idle);
        // 對話不會被移除
        assert_eq!(sm.session_count(), 4);
        assert!(!sm.should_exit());
    }

    #[test]
    fn test_expire_stale_keeps_recent_sessions() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "running");
        let almost = Instant::now() + STALE_TIMEOUT - Duration::from_secs(60);
        assert!(!sm.expire_stale(almost, STALE_TIMEOUT));
        assert_eq!(sm.current_state(), LightState::Running);
    }

    #[test]
    fn test_new_event_refreshes_stale_timer() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "running");
        std::thread::sleep(Duration::from_millis(20));
        let checkpoint = Instant::now();
        sm.handle_event("s1", "running");
        // 以最後一次事件為準：從 checkpoint 起算還不到 timeout
        assert!(!sm.expire_stale(checkpoint + STALE_TIMEOUT - Duration::from_millis(10), STALE_TIMEOUT));
        assert_eq!(sm.current_state(), LightState::Running);
    }

    #[test]
    fn folder_name_takes_last_segment() {
        assert_eq!(folder_name(r"C:\Users\me\pawpeek").as_deref(), Some("pawpeek"));
        assert_eq!(folder_name("/home/me/my app/").as_deref(), Some("my app"));
        assert_eq!(folder_name(r"c:\work\viacasino-fe\").as_deref(), Some("viacasino-fe"));
        assert_eq!(folder_name(r"C:\"), None);
        assert_eq!(folder_name("/"), None);
        assert_eq!(folder_name("  "), None);
    }

    #[test]
    fn folder_follows_the_session_that_changed_state() {
        let mut sm = StateMachine::new();
        sm.handle_event_in("a", "running", Some(r"C:\work\alpha"));
        sm.handle_event_in("b", "running", Some(r"C:\work\beta"));
        assert_eq!(sm.current_folder().as_deref(), Some("alpha")); // 第一個讓狀態變成工作中的
        // b 要求確認 → 等待確認，是 b 造成的
        sm.handle_event_in("b", "need_confirm", None);
        assert_eq!(sm.current_state(), LightState::NeedConfirm);
        assert_eq!(sm.current_folder().as_deref(), Some("beta"));
        // b 完成 → 回到工作中，但工作中的是 a（不是觸發的 b）
        sm.handle_event("b", "stop");
        assert_eq!(sm.current_state(), LightState::Running);
        assert_eq!(sm.current_folder().as_deref(), Some("alpha"));
        // a 也完成 → 待機，是 a 造成的
        sm.handle_event("a", "stop");
        assert_eq!(sm.current_state(), LightState::Idle);
        assert_eq!(sm.current_folder().as_deref(), Some("alpha"));
    }

    #[test]
    fn folder_unknown_when_ambiguous_or_missing() {
        let mut sm = StateMachine::new();
        sm.handle_event_in("a", "running", Some(r"C:\work\alpha"));
        sm.handle_event_in("b", "running", Some(r"C:\work\beta"));
        sm.handle_event("a", "stop");
        // 逾時一起回到待機：兩個對話都在待機，分不出是誰
        assert!(sm.expire_stale(Instant::now() + STALE_TIMEOUT, STALE_TIMEOUT));
        assert_eq!(sm.current_state(), LightState::Idle);
        assert_eq!(sm.current_folder(), None);
        // 沒有 cwd 的對話
        sm.handle_event("c", "need_confirm");
        assert_eq!(sm.current_folder(), None);
    }

    #[test]
    fn test_two_sessions_end_independently() {
        let mut sm = StateMachine::new();
        sm.handle_event("s1", "session_start");
        sm.handle_event("s2", "session_start");
        assert_eq!(sm.session_count(), 2);
        sm.handle_event("s1", "session_end");
        assert_eq!(sm.session_count(), 1);
        assert!(!sm.should_exit());
        sm.handle_event("s2", "session_end");
        assert_eq!(sm.session_count(), 0);
        assert!(sm.should_exit());
    }
}
