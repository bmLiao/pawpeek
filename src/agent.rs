//! 偵測 agent（Claude Code / Copilot）程序是否還在：
//! agent 沒送 SessionEnd 就結束（關掉 VS Code、終端機）時，桌寵靠這個知道對話已經結束。
//!
//! - hook 端：`find_agent_pid` 往上找第一個不是殼層（powershell、cmd、bash…）的上層程序，附在事件裡送給桌寵
//! - 桌寵端：`start_time` 記下它的啟動時間，`is_alive` 定期檢查（比對啟動時間，避免 PID 被重複使用時誤判）
//!
//! Windows 以外目前不偵測（`find_agent_pid` 回傳 None，對話只靠 SessionEnd 與逾時）。

/// hook 與 agent 之間可能經過的中間程序（小寫）：跳過它們，找真正的 agent
#[cfg_attr(not(windows), allow(dead_code))]
const SHELLS: &[&str] = &[
    "powershell.exe", "pwsh.exe", "cmd.exe", "conhost.exe",
    "sh.exe", "bash.exe", "dash.exe", "env.exe",
    "pawpeek.exe",
];

/// 是否為中間殼層（不是 agent）
#[cfg_attr(not(windows), allow(dead_code))]
fn is_shell(exe: &str) -> bool {
    SHELLS.contains(&exe.to_ascii_lowercase().as_str())
}

/// 從 `start` 的上層開始往上找第一個不是殼層的程序。`parent_of` 回傳 (上層 PID, 上層執行檔名)。
#[cfg_attr(not(windows), allow(dead_code))]
fn walk_to_agent(start: u32, parent_of: impl Fn(u32) -> Option<(u32, String)>) -> Option<u32> {
    let mut pid = start;
    for _ in 0..8 {
        let (ppid, exe) = parent_of(pid)?;
        if ppid == 0 { return None; }
        if !is_shell(&exe) { return Some(ppid); }
        pid = ppid;
    }
    None
}

#[cfg(windows)]
mod win {
    #[repr(C)]
    pub struct ProcessEntry32W {
        pub dw_size: u32,
        pub cnt_usage: u32,
        pub th32_process_id: u32,
        pub th32_default_heap_id: usize,
        pub th32_module_id: u32,
        pub cnt_threads: u32,
        pub th32_parent_process_id: u32,
        pub pc_pri_class_base: i32,
        pub dw_flags: u32,
        pub sz_exe_file: [u16; 260],
    }
    #[repr(C)]
    #[derive(Default)]
    pub struct FileTime { pub low: u32, pub high: u32 }
    extern "system" {
        pub fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        pub fn Process32FirstW(snap: isize, entry: *mut ProcessEntry32W) -> i32;
        pub fn Process32NextW(snap: isize, entry: *mut ProcessEntry32W) -> i32;
        pub fn CloseHandle(h: isize) -> i32;
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        pub fn GetProcessTimes(h: isize, c: *mut FileTime, e: *mut FileTime, k: *mut FileTime, u: *mut FileTime) -> i32;
        pub fn GetExitCodeProcess(h: isize, code: *mut u32) -> i32;
        pub fn GetLastError() -> u32;
    }
    pub const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
}

/// 所有程序的 PID → (上層 PID, 執行檔名)
#[cfg(windows)]
fn process_table() -> std::collections::HashMap<u32, (u32, String)> {
    use win::*;
    let mut map = std::collections::HashMap::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(0x2, 0); // TH32CS_SNAPPROCESS
        if snap == -1 || snap == 0 { return map; }
        let mut e: ProcessEntry32W = std::mem::zeroed();
        e.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(e.sz_exe_file.len());
            map.insert(e.th32_process_id, (e.th32_parent_process_id, String::from_utf16_lossy(&e.sz_exe_file[..len])));
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    map
}

/// hook 端：找出執行這個 hook 的 agent 程序（跳過中間的殼層）
pub fn find_agent_pid() -> Option<u32> {
    #[cfg(windows)]
    {
        let table = process_table();
        walk_to_agent(std::process::id(), |pid| {
            let (ppid, _) = table.get(&pid)?;
            let exe = table.get(ppid).map(|(_, n)| n.clone()).unwrap_or_default();
            Some((*ppid, exe))
        })
    }
    #[cfg(not(windows))]
    { None }
}

/// 程序的啟動時間（拿不到回傳 None）
pub fn start_time(pid: u32) -> Option<u64> {
    #[cfg(windows)]
    unsafe {
        use win::*;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 { return None; }
        let (mut c, mut e, mut k, mut u) = (FileTime::default(), FileTime::default(), FileTime::default(), FileTime::default());
        let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u);
        CloseHandle(h);
        if ok == 0 { return None; }
        Some(((c.high as u64) << 32) | c.low as u64)
    }
    #[cfg(not(windows))]
    { let _ = pid; None }
}

/// 程序是否還在（`start` 是記錄時的啟動時間；不同代表 PID 已被別的程序重複使用）。
/// 無法判斷（例如權限不足）時當作還在，避免誤把對話結束。
pub fn is_alive(pid: u32, start: Option<u64>) -> bool {
    #[cfg(windows)]
    unsafe {
        use win::*;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return GetLastError() != 87; // ERROR_INVALID_PARAMETER：沒有這個 PID
        }
        let mut code = 0u32;
        let running = GetExitCodeProcess(h, &mut code) != 0 && code == 259; // STILL_ACTIVE
        CloseHandle(h);
        if !running { return false; }
        match (start, start_time(pid)) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
    #[cfg(not(windows))]
    { let _ = (pid, start); true }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain<'a>(links: &'a [(u32, u32, &'a str)]) -> impl Fn(u32) -> Option<(u32, String)> + 'a {
        // (pid, ppid, 上層的執行檔名)
        move |pid| links.iter().find(|l| l.0 == pid).map(|l| (l.1, l.2.to_string()))
    }

    #[test]
    fn skips_shells_to_agent() {
        // VS Code Copilot：copilot-runtime.exe → powershell.exe → pawpeek.exe（hook）
        let links = [(100, 50, "powershell.exe"), (50, 10, "copilot-runtime.exe")];
        assert_eq!(walk_to_agent(100, chain(&links)), Some(10));
    }

    #[test]
    fn direct_parent_is_agent() {
        // Claude Code：claude.exe → pawpeek.exe
        let links = [(100, 7, "claude.exe")];
        assert_eq!(walk_to_agent(100, chain(&links)), Some(7));
    }

    #[test]
    fn shell_names_are_case_insensitive() {
        let links = [(100, 50, "PowerShell.EXE"), (50, 9, "node.exe")];
        assert_eq!(walk_to_agent(100, chain(&links)), Some(9));
    }

    #[test]
    fn no_agent_when_only_shells() {
        let links = [(100, 50, "cmd.exe"), (50, 0, "")];
        assert_eq!(walk_to_agent(100, chain(&links)), None);
    }

    #[cfg(windows)]
    #[test]
    fn current_process_is_alive() {
        let me = std::process::id();
        let t = start_time(me);
        assert!(t.is_some());
        assert!(is_alive(me, t));
        assert!(!is_alive(me, t.map(|x| x + 1))); // 啟動時間不同 → 視為別的程序
    }
}
