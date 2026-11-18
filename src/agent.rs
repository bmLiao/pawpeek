//! 偵測 agent（Claude Code / Copilot）程序是否還在：
//! agent 沒送 SessionEnd 就結束（關掉 VS Code、終端機）時，桌寵靠這個知道對話已經結束。
//!
//! - hook 端：`find_agent_pid` 往上找第一個不是殼層（powershell、cmd、bash…）的上層程序，附在事件裡送給桌寵
//! - 桌寵端：`start_time` 記下它的啟動時間，`is_alive` 定期檢查（比對啟動時間，避免 PID 被重複使用時誤判）
//!
//! - Windows：Toolhelp 快照、OpenProcess / GetProcessTimes
//! - macOS / Linux：`ps` 取得程序表與啟動時間，`kill(pid, 0)` 判斷程序是否還在
//!
//! 其他平台不偵測（`find_agent_pid` 回傳 None，對話只靠 SessionEnd 與逾時）。

/// hook 與 agent 之間可能經過的中間程序（小寫）：跳過它們，找真正的 agent
#[cfg_attr(not(any(windows, unix)), allow(dead_code))]
const SHELLS: &[&str] = &[
    "powershell.exe", "pwsh.exe", "cmd.exe", "conhost.exe",
    "sh.exe", "bash.exe", "dash.exe", "env.exe",
    "pawpeek.exe",
    // macOS / Linux：hook 經 `sh -c` 執行，啟動器 bin/pawpeek 以 exec 轉成 pawpeek-macos
    "sh", "bash", "zsh", "dash", "fish", "env",
    "pawpeek", "pawpeek-macos",
];

/// 是否為中間殼層（不是 agent）
#[cfg_attr(not(any(windows, unix)), allow(dead_code))]
fn is_shell(exe: &str) -> bool {
    SHELLS.contains(&exe.to_ascii_lowercase().as_str())
}

/// 從 `start` 的上層開始往上找第一個不是殼層的程序。`parent_of` 回傳 (上層 PID, 上層執行檔名)。
#[cfg_attr(not(any(windows, unix)), allow(dead_code))]
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

/// 所有程序的 PID → (上層 PID, 執行檔名)。`ps` 的 comm 可能是完整路徑（含空白），只取檔名
#[cfg(unix)]
fn process_table() -> std::collections::HashMap<u32, (u32, String)> {
    let mut map = std::collections::HashMap::new();
    let Ok(out) = std::process::Command::new("ps").args(["-axo", "pid=,ppid=,comm="])
        .stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output() else { return map };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some((pid, ppid, comm)) = parse_ps_line(line) {
            map.insert(pid, (ppid, comm));
        }
    }
    map
}

/// `ps -o pid=,ppid=,comm=` 的一行 → (PID, 上層 PID, 執行檔名)
#[cfg_attr(not(unix), allow(dead_code))]
fn parse_ps_line(line: &str) -> Option<(u32, u32, String)> {
    let line = line.trim_start();
    let (pid, rest) = line.split_once(char::is_whitespace)?;
    let rest = rest.trim_start();
    let (ppid, comm) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    let comm = comm.trim();
    let name = comm.rsplit('/').next().unwrap_or(comm);
    Some((pid.parse().ok()?, ppid.parse().ok()?, name.to_string()))
}

#[cfg(unix)]
extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
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
    #[cfg(unix)]
    {
        let table = process_table();
        walk_to_agent(std::process::id(), |pid| {
            let (ppid, _) = table.get(&pid)?;
            let exe = table.get(ppid).map(|(_, n)| n.clone()).unwrap_or_default();
            Some((*ppid, exe))
        })
    }
    #[cfg(not(any(windows, unix)))]
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
    // `ps -o lstart=` 是秒級的啟動時刻文字（例：Wed Nov 18 10:00:00 2026），雜湊成數字比對
    #[cfg(unix)]
    {
        let out = std::process::Command::new("ps").args(["-o", "lstart=", "-p", &pid.to_string()])
            .stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
        let text = String::from_utf8_lossy(&out.stdout);
        let text = text.trim();
        if !out.status.success() || text.is_empty() { return None; }
        // FNV-1a：固定演算法，同一段文字永遠得到同一個值
        Some(text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3)))
    }
    #[cfg(not(any(windows, unix)))]
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
    #[cfg(unix)]
    {
        if pid == 0 || pid > i32::MAX as u32 { return true; }
        if unsafe { kill(pid as i32, 0) } != 0 {
            // ESRCH（3，macOS 與 Linux 相同）：沒有這個 PID；EPERM 等其他錯誤代表程序還在
            return std::io::Error::last_os_error().raw_os_error() != Some(3);
        }
        match (start, start_time(pid)) {
            (Some(a), Some(b)) => a == b,
            _ => true,
        }
    }
    #[cfg(not(any(windows, unix)))]
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

    #[test]
    fn unix_shells_are_skipped() {
        // macOS Claude Code：claude → sh -c（hook）→ pawpeek-macos
        let links = [(100, 50, "sh"), (50, 7, "claude")];
        assert_eq!(walk_to_agent(100, chain(&links)), Some(7));
        let links = [(100, 50, "zsh"), (50, 40, "env"), (40, 9, "node")];
        assert_eq!(walk_to_agent(100, chain(&links)), Some(9));
    }

    #[test]
    fn parses_ps_lines() {
        assert_eq!(parse_ps_line("  123     1 /bin/zsh"), Some((123, 1, "zsh".into())));
        assert_eq!(
            parse_ps_line("456 123 /Applications/Visual Studio Code.app/Contents/MacOS/Electron"),
            Some((456, 123, "Electron".into()))
        );
        assert_eq!(parse_ps_line("789 1 node"), Some((789, 1, "node".into())));
        assert_eq!(parse_ps_line("garbage"), None);
    }

    #[cfg(any(windows, unix))]
    #[test]
    fn current_process_is_alive() {
        let me = std::process::id();
        let t = start_time(me);
        assert!(t.is_some());
        assert!(is_alive(me, t));
        assert!(!is_alive(me, t.map(|x| x + 1))); // 啟動時間不同 → 視為別的程序
    }
}
