use std::env;
use std::io::{self, Read};
use std::time::Duration;
use pawpeek::client::{send_event_str, send_raw_json};

use pawpeek::daemon::{run_daemon, start_detached_daemon, fixed_port, is_manual_quit, clear_manual_quit};

fn read_stdin_and_send(port: u16) {
    let mut stdin_bytes = Vec::new();
    if io::stdin().read_to_end(&mut stdin_bytes).is_err() {
        return;
    }
    if stdin_bytes.is_empty() || stdin_bytes.iter().all(|&b| b.is_ascii_whitespace()) {
        return;
    }

    // 用户手动退出后，只有新会话开始才允许重新启动
    if is_manual_quit() {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&stdin_bytes) {
            if v.get("hook_event_name").and_then(|n| n.as_str()) == Some("SessionStart") {
                clear_manual_quit(); // 新会话：允许重启
            } else {
                return; // 其他 hook 不重启
            }
        } else {
            return;
        }
    }

    // Try sending; if daemon not running, start it and poll up to ~2s
    let mut daemon_started = false;
    for _ in 0..20 {
        match send_raw_json(port, &stdin_bytes) {
            Ok(()) => return,
            Err(ref e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                if !daemon_started {
                    start_detached_daemon(port);
                    daemon_started = true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

/// 送外部指令給桌寵；沒在跑就啟動它（使用者主動操作：手動離開過也允許重新啟動）
fn send_cmd(port: u16, json: &[u8]) -> bool {
    clear_manual_quit();
    allow_foreground();
    let mut daemon_started = false;
    for _ in 0..50 {
        match send_raw_json(port, json) {
            Ok(()) => return true,
            Err(ref e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                if !daemon_started {
                    start_detached_daemon(port);
                    daemon_started = true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => { eprintln!("無法連線到桌寵：{e}"); return false; }
        }
    }
    false
}

/// `--import-skin`：請桌寵跳出匯入外觀視窗（實際流程在 daemon 內處理）
fn import_skin(port: u16) {
    if send_cmd(port, br#"{"cmd":"import_skin"}"#) {
        println!("已通知桌寵：請在跳出的視窗選擇寵物圖片（png / webp），預覽後再決定是否保留。");
        return;
    }
    eprintln!("桌寵沒有回應，請稍後再試。");
    std::process::exit(1);
}

/// `--set-skin [值]`：換外觀。值可以是 Petdex 名稱、圖片路徑或 default；
/// 不帶值則列出目前設定與本機已有的寵物。
fn set_skin(port: u16, value: &str) {
    if value.trim().is_empty() {
        let env = pawpeek::config::load_env();
        let cur = env.get("PET_SKIN").map(|s| s.trim()).filter(|s| !s.is_empty()).unwrap_or("（內建預設）");
        println!("目前外觀：{cur}");
        let pets = pawpeek::skin::list_local();
        if pets.is_empty() {
            println!("本機還沒有其他寵物。");
        } else {
            println!("本機可用：{}", pets.join("、"));
        }
        println!("用法：--set-skin <Petdex 名稱 | 圖片路徑 | default>");
        return;
    }
    pawpeek::config::ensure_files();
    match pawpeek::skin::apply(value) {
        Ok(msg) => {
            println!("{msg}");
            if !send_cmd(port, br#"{"cmd":"reload_skin"}"#) {
                println!("（設定已存好，但桌寵沒有回應，下次啟動會套用。）");
            }
        }
        Err(e) => {
            eprintln!("換外觀失敗：{e}");
            std::process::exit(1);
        }
    }
}

/// Windows：允許桌寵把視窗帶到前景（否則選檔視窗可能被壓在後面）
fn allow_foreground() {
    #[cfg(target_os = "windows")]
    unsafe {
        extern "system" { fn AllowSetForegroundWindow(pid: u32) -> i32; }
        AllowSetForegroundWindow(u32::MAX); // ASFW_ANY
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let port = fixed_port();

    if args.len() >= 2 && args[1] == "--daemon" {
        match run_daemon(port) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    if args.len() >= 2 && args[1] == "--import-skin" {
        import_skin(port);
        return;
    }

    if args.len() >= 2 && args[1] == "--quit" {
        // 沒在跑就什麼都不做（不要為了關閉而啟動它）
        match send_raw_json(port, br#"{"cmd":"quit"}"#) {
            Ok(()) => println!("桌寵已離開，下次開新的 Claude Code 對話時會再出現。"),
            Err(_) => println!("桌寵目前沒有在執行。"),
        }
        return;
    }

    if args.len() >= 2 && args[1] == "--set-skin" {
        // 名稱可能含空白：其餘參數全部接起來
        set_skin(port, &args[2..].join(" "));
        return;
    }

    if args.len() >= 2 && args[1] == "--hook" {
        read_stdin_and_send(port);
        return;
    }

    // Legacy: --event <name>
    let session_id = env::var("CLAUDE_SESSION_ID").unwrap_or_else(|_| "default_session".to_string());
    let event = if args.len() >= 3 && args[1] == "--event" {
        &args[2]
    } else {
        "idle"
    };

    let mut daemon_started = false;
    for _ in 0..20 {
        match send_event_str(port, event, &session_id) {
            Ok(()) => return,
            Err(ref e) if e.kind() == io::ErrorKind::ConnectionRefused => {
                if !daemon_started {
                    start_detached_daemon(port);
                    daemon_started = true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}
