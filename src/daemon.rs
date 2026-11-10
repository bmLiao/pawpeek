use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use tao::event::Event;
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

use crate::config::{fill, Messages, SkinSource};
use crate::server::start_server;
use crate::state::LightState;
use crate::state::StateMachine;

const FIXED_PORT: u16 = 47390;

#[derive(Debug, Clone)]
pub enum UiCommand {
    SetState { state: LightState, duration_ms: Option<u64> },
    /// 換上新的 spritesheet（例如 Petdex 下載完成）
    SetSprite(Vec<u8>),
    /// 外部指令：匯入自訂外觀（`pawpeek --import-skin`）
    ImportSkin,
    /// 讓 WebView 取得鍵盤焦點（點寵物後可用 +/- 縮放）
    /// 檢查寵物是否還在某個螢幕上（拔掉螢幕後救回來）
    CheckOnScreen,
    /// 外部指令：依 .env 重新載入外觀（`pawpeek --set-skin` 改完 .env 後送來）
    ReloadSkin,
    /// 在寵物頁執行的 JS（例如泡泡改到寵物下方）
    PetJs(String),
    IdleAction { action_state: &'static str, bubble: String },
    SessionCount(u32),
    ShowBubble { text: String, duration_ms: u64 },
    Greeting,
    Goodbye,
    Quit,
}

fn state_name(s: LightState) -> &'static str {
    match s {
        LightState::Idle => "idle",
        LightState::Running => "running",
        LightState::NeedConfirm => "waiting",
        LightState::ToolError => "review",
        LightState::ErrorFinal => "failed",
    }
}

fn bubble_key(s: LightState) -> Option<&'static str> {
    match s {
        LightState::Running => Some("state_bubble.running"),
        LightState::NeedConfirm => Some("state_bubble.need_confirm"),
        LightState::ToolError => Some("state_bubble.tool_error"),
        LightState::ErrorFinal => Some("state_bubble.error_final"),
        LightState::Idle => None,
    }
}

/// Rust 字串 → JS 字串字面值（避免使用者文字中的引號弄壞腳本）
fn js_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".into())
}

/// WebView2 的畫面程式（browser / renderer）崩潰或無回應時，結束主程式。
/// 不設 `.manual-quit`，所以下一個 hook 送來時會自動重新啟動 daemon。
#[cfg(target_os = "windows")]
fn watch_webview_crash(webview: &wry::WebView, proxy: tao::event_loop::EventLoopProxy<UiCommand>) {
    use webview2_com::Microsoft::Web::WebView2::Win32::*;
    use webview2_com::ProcessFailedEventHandler;
    use wry::WebViewExtWindows;

    let handler = ProcessFailedEventHandler::create(Box::new(move |_, args| {
        let mut kind = COREWEBVIEW2_PROCESS_FAILED_KIND_UNKNOWN_PROCESS_EXITED;
        if let Some(args) = args {
            unsafe { let _ = args.ProcessFailedKind(&mut kind); }
        }
        // GPU / 子 frame / 外掛等程式掛掉 WebView2 會自行復原，不需結束
        let fatal = kind == COREWEBVIEW2_PROCESS_FAILED_KIND_BROWSER_PROCESS_EXITED
            || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_EXITED
            || kind == COREWEBVIEW2_PROCESS_FAILED_KIND_RENDER_PROCESS_UNRESPONSIVE;
        if fatal && proxy.send_event(UiCommand::Quit).is_err() {
            std::process::exit(1);
        }
        Ok(())
    }));
    unsafe {
        let mut token = 0i64;
        match webview.controller().CoreWebView2() {
            Ok(core) => {
                if let Err(e) = core.add_ProcessFailed(&handler, &mut token) {
                    eprintln!("[pawpeek] 無法監聽 WebView2 崩潰事件: {e}");
                }
            }
            Err(e) => eprintln!("[pawpeek] 取得 CoreWebView2 失敗: {e}"),
        }
    }
}

pub fn run_daemon(port: u16) -> Result<(), String> {
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .map_err(|_| format!("Port {} already in use", port))?;
    let state = Arc::new(Mutex::new(StateMachine::new()));
    #[cfg_attr(not(target_os = "macos"), allow(unused_mut))] // macOS 的 set_activation_policy 需要 &mut
    let mut event_loop = EventLoopBuilder::<UiCommand>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let state_srv = Arc::clone(&state);
    let proxy_cmd = std::sync::Mutex::new(proxy.clone());
    let on_cmd: crate::server::CmdHandler = Arc::new(move |cmd: &str| {
        let ui = match cmd {
            "import_skin" => UiCommand::ImportSkin,
            "reload_skin" => UiCommand::ReloadSkin,
            // 手動離開：下次新 session 開始才會再出現
            "quit" => { mark_manual_quit(); UiCommand::Quit }
            _ => return,
        };
        if let Ok(p) = proxy_cmd.lock() { let _ = p.send_event(ui); }
    });
    std::thread::spawn(move || { start_server(listener, state_srv, on_cmd); });

    // Window
    let mut window_builder = WindowBuilder::new()
        .with_inner_size(tao::dpi::LogicalSize::new(140.0, 180.0))
        .with_decorations(false).with_transparent(true).with_always_on_top(true)
        .with_resizable(false);
    #[cfg(target_os = "windows")]
    { use tao::platform::windows::WindowBuilderExtWindows;
      window_builder = window_builder.with_skip_taskbar(true).with_no_redirection_bitmap(true); }
    let window = window_builder.build(&event_loop).expect("window");

    // Windows: 强制移除所有边框/标题栏样式，解决 Windows 11 顶部白线
    #[cfg(target_os = "windows")]
    {
        use tao::platform::windows::WindowExtWindows;
        unsafe {
            extern "system" {
                fn SetWindowLongPtrW(hwnd: isize, index: i32, new_long: isize) -> isize;
                fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
                fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
                fn CallWindowProcW(prev: isize, hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
            }
            let hwnd = window.hwnd();
            let _ = SCREEN_PROXY.set(Mutex::new(proxy.clone()));
            PET_HWND.store(hwnd as isize, std::sync::atomic::Ordering::SeqCst);
            let style = GetWindowLongPtrW(hwnd, -16);
            // 去掉边框、标题栏、以及最大化/最小化/系统菜单能力
            // ~(WS_BORDER|WS_THICKFRAME|WS_CAPTION|WS_MINIMIZEBOX|WS_MAXIMIZEBOX|WS_SYSMENU)
            SetWindowLongPtrW(hwnd, -16, style & !((0x00800000_i32 | 0x00040000 | 0x00C00000 | 0x00020000 | 0x00010000 | 0x00080000) as isize));
            let ex_style = GetWindowLongPtrW(hwnd, -20);
            // 保留原有移除项 + 添加 WS_EX_NOACTIVATE 防止启动时抢焦点
            SetWindowLongPtrW(hwnd, -20, (ex_style & !((0x00000001_i32 | 0x00000200 | 0x00020000) as isize)) | 0x08000000);
            SetWindowPos(hwnd, 0, 0, 0, 0, 0, 0x0020 | 0x0002 | 0x0001 | 0x0004);

            // Subclass WndProc：拦截 WM_SYSCOMMAND 的最大化/最小化/还原
            // （样式位只隐藏按钮，ShowWindow/UI Automation 仍可触发，必须在消息层拦截）
            static PREV_PROC: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);
            extern "system" fn wnd_proc(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize {
                const WM_SYSCOMMAND: u32 = 0x0112;
                const SC_MINIMIZE: usize = 0xF020;
                const SC_MAXIMIZE: usize = 0xF030;
                const SC_RESTORE: usize = 0xF120;
                // 螢幕接上 / 拔掉 / 改配置：檢查寵物是否還在某個螢幕上
                const WM_DISPLAYCHANGE: u32 = 0x007E;
                if msg == WM_DISPLAYCHANGE {
                    if let Some(p) = SCREEN_PROXY.get() {
                        if let Ok(p) = p.lock() { let _ = p.send_event(UiCommand::CheckOnScreen); }
                    }
                }
                if msg == WM_SYSCOMMAND {
                    let cmd = wparam & 0xFFF0;
                    if cmd == SC_MINIMIZE || cmd == SC_MAXIMIZE || cmd == SC_RESTORE {
                        return 0; // 吞掉，不处理
                    }
                }
                let prev = PREV_PROC.load(std::sync::atomic::Ordering::Relaxed);
                unsafe { CallWindowProcW(prev, hwnd, msg, wparam, lparam) }
            }
            // GWLP_WNDPROC = -4
            let prev = SetWindowLongPtrW(hwnd, -4, wnd_proc as *const () as isize);
            PREV_PROC.store(prev, std::sync::atomic::Ordering::Relaxed);
        }
    }

    // 啟動位置：上次記住的位置；不在任何螢幕上（或沒記錄）才重置到
    // 作用中視窗（通常是開 Claude 的終端機）內側右下角，找不到就主螢幕中央
    {
        let active = active_window_rect();
        // 移到縮放比例不同的螢幕時 Windows 會重新調整視窗大小與位置 → 用新大小再算一次
        for _ in 0..2 {
            let sf = window.scale_factor();
            let size = window.outer_size();
            let (sw, sh) = (size.width as i32, size.height as i32);
            let (x, y) = match load_position() {
                Some((x, y)) if on_any_monitor(&window, x + sw / 2, y + sh / 2) => (x, y),
                _ => home_spot_for(&window, active, (sw, sh)),
            };
            window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
            if window.scale_factor() == sf { break; }
        }
    }

    #[cfg(target_os = "macos")]
    { use tao::platform::macos::EventLoopExtMacOS;
      event_loop.set_activation_policy(tao::platform::macos::ActivationPolicy::Accessory); }

    // Wrap window for sharing with IPC handler (Petdex-style direct drag)
    let win_arc = Arc::new(Mutex::new(window));
    let window = win_arc.clone(); // keep name `window` for later

    // 設定檔：<文件>/.pawpeek/{messages.json,.env}（沒有就產生範本）
    crate::config::ensure_files();
    let msgs = Arc::new(Messages::load());

    // 依 .env 的 PET_SKIN 載入外觀；讀不到就先用內建預設
    let (sheet, startup_bubble, pending_download) = load_skin_from_env(&msgs);
    let saved_scale: f64 = std::fs::read_to_string(data_dir().join("pet-scale"))
        .ok().and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
    let html = crate::webview::build_page(&msgs.to_js(), saved_scale);

    // Custom protocol: / 返回 HTML，/sprite 返回精灵图
    let html_bytes: Vec<u8> = html.into_bytes();
    let sprite_mime = crate::skin::mime(&sheet);
    let sprite_data: Arc<Mutex<(Vec<u8>, &'static str)>> = Arc::new(Mutex::new((sheet, sprite_mime)));
    let sprite_serve = Arc::clone(&sprite_data);

    // WebView: IPC handler moves window directly, no EventLoop roundtrip
    let drag_win = win_arc.clone();
    let proxy_ipc = proxy.clone();
    // 泡泡展開狀態：(沒有泡泡時的視窗大小（實體像素）, 泡泡是否在寵物下方)
    let frame_state: std::cell::Cell<Option<((i32, i32), bool)>> = std::cell::Cell::new(None);

    let webview = WebViewBuilder::new()
        .with_transparent(true)
        .with_custom_protocol("pet".into(), move |_id, request| {
            let path = request.uri().path();
            if path == "/sprite" {
                let guard = sprite_serve.lock().unwrap();
                let mime = guard.1;
                let bytes = guard.0.clone();
                wry::http::Response::builder()
                    .header("Content-Type", mime)
                    .header("Cache-Control", "no-store")
                    .body(std::borrow::Cow::from(bytes))
                    .unwrap()
            } else {
                wry::http::Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(std::borrow::Cow::from(html_bytes.clone()))
                    .unwrap()
            }
        })
        .with_url("pet://localhost/")
        .with_ipc_handler(move |msg| {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(msg.body()) {
                if v.get("type").and_then(|t| t.as_str()) == Some("savePos") {
                    if let Ok(w) = drag_win.lock() {
                        // 存「沒有泡泡時」的位置，重開時寵物才會在同一個地方
                        let (x, y) = match (w.outer_position(), frame_state.get()) {
                            (Ok(_), Some((base, below))) => {
                                let (x, y) = crate::bubble::base_origin(content_rect(&w), base, below);
                                let (ox, oy) = frame_offset(&w);
                                (x - ox, y - oy)
                            }
                            (Ok(pos), None) => (pos.x, pos.y),
                            _ => (0, 0),
                        };
                        save_position(x, y);
                    }
                }
                if v.get("type").and_then(|t| t.as_str()) == Some("center") {
                    if let Ok(w) = drag_win.lock() {
                        // 螢幕被拔掉時：寵物（不含泡泡）回到作用中視窗旁，找不到就主螢幕中央
                        {
                            let active = active_window_rect();
                            let cur = content_rect(&w);
                            let (mut base, below) = frame_state.get().unwrap_or(((cur.w, cur.h), false));
                            // 跨到縮放比例不同的螢幕時視窗大小會變 → 換算寵物大小後再對一次位置
                            for _ in 0..2 {
                                let sf = w.scale_factor();
                                let (bx, by) = crate::bubble::base_origin(content_rect(&w), base, below);
                                let (tx, ty) = home_spot_for(&w, active, base);
                                let out = w.outer_position().unwrap_or_default();
                                w.set_outer_position(tao::dpi::PhysicalPosition::new(out.x + tx - bx, out.y + ty - by));
                                let (ox, oy) = frame_offset(&w);
                                save_position(tx - ox, ty - oy);
                                let nsf = w.scale_factor();
                                if nsf == sf { break; }
                                let k = nsf / sf;
                                base = ((base.0 as f64 * k).round() as i32, (base.1 as f64 * k).round() as i32);
                                frame_state.set(Some((base, below)));
                            }
                        }
                    }
                }
                if v.get("type").and_then(|t| t.as_str()) == Some("saveScale") {
                    if let Some(s) = v.get("scale").and_then(|s| s.as_f64()) {
                        save_scale(s.clamp(0.5, 1.5) as f32);
                    }
                }
                if v.get("type").and_then(|t| t.as_str()) == Some("resize") {
                    let num = |k: &str, d: u64| v.get(k).and_then(|x| x.as_f64()).map(|x| x.max(0.0) as u64).unwrap_or(d).clamp(80, 800) as f64;
                    let (w, h) = (num("w", 140), num("h", 180));
                    let (bw, bh) = (num("baseW", w as u64), num("baseH", h as u64));
                    if let Ok(win) = drag_win.lock() {
                        // 寵物在螢幕上的位置不動：泡泡往上長，頂端放不下就往下長
                        let sf = win.scale_factor();
                        let px = |x: f64| (x * sf).round() as i32;
                        // 一律用內容區（inner）計算：外框可能含看不見的邊框，混用會讓寵物每次縮放都往右下偏
                        let cur = content_rect(&win);
                        let (old_base, below) = frame_state.get().unwrap_or(((cur.w, cur.h), false));
                        let screen_top = win.current_monitor().map(|m| m.position().y).unwrap_or(i32::MIN / 2);
                        let f = crate::bubble::next_frame(cur, old_base, below, (px(bw), px(bh)), (px(w), px(h)), screen_top);
                        frame_state.set(Some(((px(bw), px(bh)), f.below)));
                        if f.below != below {
                            let _ = proxy_ipc.send_event(UiCommand::PetJs(format!("setBubbleSide({})", f.below)));
                        }
                        let (ox, oy) = frame_offset(&win);
                        let _ = win.set_inner_size(tao::dpi::PhysicalSize::new(f.rect.w as u32, f.rect.h as u32));
                        win.set_outer_position(tao::dpi::PhysicalPosition::new(f.rect.x - ox, f.rect.y - oy));
                    }
                }
                if v.get("type").and_then(|t| t.as_str()) == Some("move") {
                    let dx = v.get("dx").and_then(|d| d.as_i64()).unwrap_or(0) as i32;
                    let dy = v.get("dy").and_then(|d| d.as_i64()).unwrap_or(0) as i32;
                    if let Ok(w) = drag_win.lock() {
                        let pos = w.outer_position().unwrap_or_default();
                        w.set_outer_position(tao::dpi::PhysicalPosition::new(pos.x + dx, pos.y + dy));
                    }
                }
            }
        })
        .build(&*window.lock().unwrap()).expect("webview");

    // 生命週期：畫面程式崩潰 → 主程式一起結束
    #[cfg(target_os = "windows")]
    watch_webview_crash(&webview, proxy.clone());

    // macOS: WKWebView drawsBackground = NO (must be AFTER webview build)
    #[cfg(target_os = "macos")]
    { use tao::platform::macos::WindowExtMacOS;
      let w = window.lock().unwrap();
      let ns: *mut std::ffi::c_void = w.ns_window();
      if !ns.is_null() { unsafe {
        let win: &objc::runtime::Object = &*(ns as *const objc::runtime::Object);
        let _: () = objc::msg_send![win, setOpaque: false];
        let _: () = objc::msg_send![win, setAcceptsMouseMovedEvents: true];
        let _: () = objc::msg_send![win, setMovableByWindowBackground: true];
        let c: *mut objc::runtime::Object = objc::msg_send![objc::class!(NSColor), clearColor];
        let _: () = objc::msg_send![win, setBackgroundColor: c];
        let cv: *mut objc::runtime::Object = objc::msg_send![win, contentView];
        if !cv.is_null() {
          let subs: *mut objc::runtime::Object = objc::msg_send![cv, subviews];
          if !subs.is_null() {
          let n: usize = objc::msg_send![subs, count];
          for i in 0..n {
            let v: *mut objc::runtime::Object = objc::msg_send![subs, objectAtIndex: i];
            if v.is_null() { continue; }
            let no: *mut objc::runtime::Object = objc::msg_send![objc::class!(NSNumber), numberWithBool: false];
            let k: *mut objc::runtime::Object = objc::msg_send![objc::class!(NSString), stringWithUTF8String: b"drawsBackground\0".as_ptr() as *const i8];
            let _: () = objc::msg_send![v, setValue: no forKey: k];
          }
          }
        }
      }}
    }

    { let w = window.lock().unwrap(); w.set_visible(true); }
    // 啟動時用 WS_EX_NOACTIVATE 避免搶焦點；顯示後拿掉，之後點桌寵就照系統預設取得焦點
    #[cfg(target_os = "windows")]
    {
        use tao::platform::windows::WindowExtWindows;
        extern "system" {
            fn SetWindowLongPtrW(hwnd: isize, index: i32, new_long: isize) -> isize;
            fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
        }
        let hwnd = window.lock().unwrap().hwnd() as isize;
        unsafe { SetWindowLongPtrW(hwnd, -20, GetWindowLongPtrW(hwnd, -20) & !0x08000000); }
    }

    // 啟動提示 + 需要時從 Petdex 下載外觀（稍等頁面載入完成再送氣泡）
    if startup_bubble.is_some() || pending_download.is_some() {
        let proxy_skin = proxy.clone();
        let msgs_skin = Arc::clone(&msgs);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(2000));
            let bubble = |text: String, ms: u64| {
                if !text.is_empty() {
                    let _ = proxy_skin.send_event(UiCommand::ShowBubble { text, duration_ms: ms });
                }
            };
            if let Some(text) = startup_bubble { bubble(text, 4000); }
            if let Some(name) = pending_download { download_skin(&proxy_skin, &msgs_skin, &name); }
        });
    }

    // Polling + idle timer
    let state_poll = Arc::clone(&state);
    let proxy_poll = proxy.clone();
    let msgs_poll = Arc::clone(&msgs);
    std::thread::spawn(move || {
        let mut idle_since: Option<std::time::Instant> = None;
        let mut last_action: Option<std::time::Instant> = None;
        let mut prev_count = 0u32;
        let actions = ["jumping", "waving", "review"];
        let mut tick: u64 = 0;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            tick += 1;
            let (cur, count) = {
                let s = state_poll.lock().unwrap_or_else(|e| e.into_inner());
                (s.current_state(), s.session_count())
            };
            let _ = proxy_poll.send_event(UiCommand::SetState { state: cur, duration_ms: None });
            let _ = proxy_poll.send_event(UiCommand::SessionCount(count));

            // 检测会话数变化：打招呼 / 告别
            if count > 0 && prev_count == 0 && cur == LightState::Idle {
                let _ = proxy_poll.send_event(UiCommand::Greeting);
            } else if count > prev_count && prev_count > 0 && cur == LightState::Idle {
                let _ = proxy_poll.send_event(UiCommand::Greeting);
            } else if count == 0 && prev_count > 0 && !matches!(cur, LightState::ErrorFinal) {
                let _ = proxy_poll.send_event(UiCommand::Goodbye);
            }
            prev_count = count;

            if cur != LightState::Idle {
                idle_since = None; last_action = None;
                continue;
            }
            // Idle: only run probability every 10 ticks (~10s)
            if tick % 10 != 0 { continue; }
            let now = std::time::Instant::now();
            let since = *idle_since.get_or_insert(now);
            let elapsed = now.duration_since(since).as_secs();
            if elapsed < 20 { continue; } // minimum 20s before first action
            if elapsed > 7200 { continue; } // stop after 2h

            // Cooldown: at least 20s between actions
            if let Some(t) = last_action {
                if t.elapsed().as_secs() < 20 { continue; }
            }

            // Probability decreases as idle time grows
            let prob = if elapsed < 300 { 40 }          // 0-5min: 40% per check
                  else if elapsed < 1800 { 15 }         // 5-30min: 15%
                  else { 5 };                            // 30min-2h: 5%

            let roll = {
                use std::hash::{BuildHasher, Hasher};
                let mut h = std::collections::hash_map::RandomState::new().build_hasher();
                h.write_u64(tick);
                h.finish() % 100
            };

            if roll >= prob { continue; }

            let s_idx = (elapsed as usize / 7) % actions.len();
            let _ = proxy_poll.send_event(UiCommand::IdleAction {
                action_state: actions[s_idx],
                bubble: msgs_poll.pick("idle"),
            });
            last_action = Some(now);
        }
    });

    // Event loop
    let mut last_state: Option<LightState> = None;
    let mut exit_at: Option<std::time::Instant> = None;
    let state_exit = Arc::clone(&state);
    let webview = Some(webview);
    let msgs_ui = Arc::clone(&msgs);
    let proxy_import = proxy.clone();
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::WaitUntil(std::time::Instant::now() + std::time::Duration::from_millis(5));

        let (should_exit, count) = {
            let s = state_exit.lock().unwrap_or_else(|e| e.into_inner());
            (s.should_exit(), s.session_count())
        };
        if should_exit && count == 0 {
            if exit_at.is_none() { exit_at = Some(std::time::Instant::now()); }
            if exit_at.unwrap().elapsed() >= std::time::Duration::from_secs(2) {
                *control_flow = ControlFlow::Exit; return;
            }
        } else { exit_at = None; }

        match event {
            Event::UserEvent(cmd) => match cmd {
                UiCommand::SetState { state, duration_ms } => {
                    if last_state == Some(state) { return; }
                    let prev = last_state;
                    last_state = Some(state);
                    if let Some(ref wv) = webview {
                        let sn = state_name(state);
                        let _ = wv.evaluate_script(&format!("setHookState('{}')", sn));
                        let _ = wv.evaluate_script(&format!("setState('{}',{})", sn, duration_ms.unwrap_or(0)));
                        if let Some(key) = bubble_key(state) {
                            let b = msgs_ui.text(key);
                            // 文字留空 = 不顯示氣泡（同時清掉舊的持久氣泡）
                            let _ = wv.evaluate_script(&format!("setBubble({},0,{})", js_str(&b), !b.is_empty()));
                        } else if prev == Some(LightState::Running) {
                            // 从工作切回空闲：先清空持久气泡，再展示完成气泡
                            let _ = wv.evaluate_script("setBubble('',0,false)");
                            let b = msgs_ui.pick("done");
                            if !b.is_empty() { let _ = wv.evaluate_script(&format!("setBubble({},3000,false)", js_str(&b))); }
                        } else {
                            let _ = wv.evaluate_script("setBubble('',0,false)");
                        }
                    }
                }
                UiCommand::SetSprite(bytes) => {
                    let mime = crate::skin::mime(&bytes);
                    *sprite_data.lock().unwrap() = (bytes, mime);
                    if let Some(ref wv) = webview {
                        let bust = std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .unwrap_or_default().as_millis();
                        let _ = wv.evaluate_script(&format!(
                            "loadSprite('/sprite?t={t}');", t=bust
                        ));
                    }
                }
                UiCommand::IdleAction { action_state, bubble } => {
                    if let Some(ref wv) = webview {
                        let _ = wv.evaluate_script(&format!("setState('{}',5000)", action_state));
                        if !bubble.is_empty() { let _ = wv.evaluate_script(&format!("setBubble({},4000,false)", js_str(&bubble))); }
                    }
                }
                UiCommand::SessionCount(n) => {
                    if let Some(ref wv) = webview {
                        let _ = wv.evaluate_script(&format!("window.__sessions={}", n));
                    }
                }
                UiCommand::ShowBubble { text, duration_ms } => {
                    if let Some(ref wv) = webview {
                        let _ = wv.evaluate_script(&format!("setBubble({},{},false)", js_str(&text), duration_ms));
                    }
                }
                UiCommand::Greeting => {
                    let b = msgs_ui.pick("greeting");
                    if let (Some(wv), false) = (webview.as_ref(), b.is_empty()) {
                        let _ = wv.evaluate_script(&format!("setBubble({},3000,false)", js_str(&b)));
                    }
                }
                UiCommand::Goodbye => {
                    let b = msgs_ui.pick("goodbye");
                    if let (Some(wv), false) = (webview.as_ref(), b.is_empty()) {
                        let _ = wv.evaluate_script(&format!("setBubble({},4000,false)", js_str(&b)));
                    }
                }
                UiCommand::ImportSkin => {
                    use std::sync::atomic::Ordering;
                    // 同時只跑一個匯入流程
                    if !IMPORTING.swap(true, Ordering::SeqCst) {
                        let proxy = proxy_import.clone();
                        let msgs = Arc::clone(&msgs_ui);
                        let original = sprite_data.lock().unwrap().0.clone();
                        std::thread::spawn(move || {
                            import_skin_flow(&proxy, &msgs, original);
                            IMPORTING.store(false, Ordering::SeqCst);
                        });
                    }
                }
                UiCommand::ReloadSkin => {
                    let proxy = proxy_import.clone();
                    let msgs = Arc::clone(&msgs_ui);
                    std::thread::spawn(move || {
                        let (sheet, warn, pending) = load_skin_from_env(&msgs);
                        let _ = proxy.send_event(UiCommand::SetSprite(sheet));
                        let text = match (warn, &pending) {
                            (Some(w), _) => w,
                            (None, None) => msgs.text("skin.changed"),
                            (None, Some(_)) => String::new(),
                        };
                        if !text.is_empty() { let _ = proxy.send_event(UiCommand::ShowBubble { text, duration_ms: 3000 }); }
                        if let Some(name) = pending { download_skin(&proxy, &msgs, &name); }
                    });
                }
                UiCommand::PetJs(js) => { if let Some(ref wv) = webview { let _ = wv.evaluate_script(&js); } }
                UiCommand::CheckOnScreen => {
                    let lost = window.lock().map(|w| {
                        let pos = w.outer_position().unwrap_or_default();
                        let size = w.outer_size();
                        !on_any_monitor(&w, pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2)
                    }).unwrap_or(false);
                    if lost {
                        if let Some(ref wv) = webview {
                            // 置中流程在 IPC 處理（需要泡泡狀態）
                            let _ = wv.evaluate_script("window.ipc.postMessage(JSON.stringify({type:'center'}))");
                        }
                    }
                }
                UiCommand::Quit => *control_flow = ControlFlow::Exit,
            },
            Event::WindowEvent { event: tao::event::WindowEvent::CloseRequested, .. } => *control_flow = ControlFlow::Exit,
            // 視窗被直接銷毀（未經 CloseRequested）也結束程式
            Event::WindowEvent { event: tao::event::WindowEvent::Destroyed, .. } => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });

    // event_loop.run never returns; Ok(()) is unreachable
    #[allow(unreachable_code)]
    Ok(())
}

pub fn start_detached_daemon(_port: u16) -> bool {
    if is_manual_quit() { return false; }
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap_or_default());
    cmd.arg("--daemon").stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; cmd.creation_flags(0x0000_0200 | 0x0000_0008); }
    cmd.spawn().is_ok()
}
pub fn fixed_port() -> u16 { FIXED_PORT }

pub fn is_manual_quit() -> bool {
    data_dir().join(".manual-quit").exists()
}

pub fn clear_manual_quit() {
    let _ = std::fs::remove_file(data_dir().join(".manual-quit"));
}

fn mark_manual_quit() {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(".manual-quit"), "");
}

/// 給 WndProc（WM_DISPLAYCHANGE）通知主迴圈用
#[cfg(target_os = "windows")]
static SCREEN_PROXY: std::sync::OnceLock<Mutex<tao::event_loop::EventLoopProxy<UiCommand>>> = std::sync::OnceLock::new();

static IMPORTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
/// 桌寵視窗的 HWND：給系統對話框當擁有者，對話框才會蓋在最上層而不是只在工作列閃
static PET_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "windows")]
struct PetParent(std::num::NonZeroIsize);

#[cfg(target_os = "windows")]
impl raw_window_handle::HasWindowHandle for PetParent {
    fn window_handle(&self) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let raw = raw_window_handle::RawWindowHandle::Win32(raw_window_handle::Win32WindowHandle::new(self.0));
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

#[cfg(target_os = "windows")]
impl raw_window_handle::HasDisplayHandle for PetParent {
    fn display_handle(&self) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let raw = raw_window_handle::RawDisplayHandle::Windows(raw_window_handle::WindowsDisplayHandle::new());
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw) })
    }
}

/// 對話框的擁有者視窗（Windows 以外回傳 None）
#[cfg(target_os = "windows")]
fn pet_parent() -> Option<PetParent> {
    std::num::NonZeroIsize::new(PET_HWND.load(std::sync::atomic::Ordering::SeqCst)).map(PetParent)
}

macro_rules! with_parent {
    ($dialog:expr) => {{
        #[allow(unused_mut)]
        let mut d = $dialog;
        #[cfg(target_os = "windows")]
        if let Some(p) = pet_parent() { d = d.set_parent(&p); }
        d
    }};
}

/// 匯入自訂外觀：選檔 → 檢查 → 預覽 → 確認保留 → 存檔並寫入 .env
/// 格式有誤就不換圖；按「否」或存檔失敗就換回 `original`
fn import_skin_flow(proxy: &tao::event_loop::EventLoopProxy<UiCommand>, msgs: &Messages, original: Vec<u8>) {
    use rfd::{AsyncFileDialog, AsyncMessageDialog, MessageButtons, MessageDialogResult, MessageLevel};
    let title = msgs.text("import.title");
    let alert = |text: String| {
        pollster::block_on(
            with_parent!(AsyncMessageDialog::new())
                .set_title(&title)
                .set_description(text)
                .set_level(MessageLevel::Warning)
                .set_buttons(MessageButtons::Ok)
                .show(),
        );
    };

    let picked = pollster::block_on(
        with_parent!(AsyncFileDialog::new())
            .set_title(msgs.text("import.pick_title"))
            .add_filter("png / webp", &["png", "webp"])
            .pick_file(),
    );
    let Some(file) = picked else { return }; // 取消
    let path = file.path().to_path_buf();
    let file_name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return alert(fill(&msgs.text("import.read_error"), &[("name", file_name.as_str())])),
    };
    let rows = match crate::skin::validate_sheet(&bytes) {
        Ok(r) => r,
        Err(crate::skin::SheetError::NotImage) => {
            return alert(fill(&msgs.text("import.not_image"), &[("name", file_name.as_str())]))
        }
        Err(crate::skin::SheetError::BadSize { w, h }) => {
            let (w, h) = (w.to_string(), h.to_string());
            return alert(fill(&msgs.text("import.bad_size"), &[("w", w.as_str()), ("h", h.as_str())]));
        }
    };

    let slug = crate::skin::CUSTOM_SLUG;
    let _ = proxy.send_event(UiCommand::SetSprite(bytes.clone()));
    let rows_s = rows.to_string();
    let answer = pollster::block_on(
        with_parent!(AsyncMessageDialog::new())
            .set_title(&title)
            .set_description(fill(&msgs.text("import.confirm"), &[("name", file_name.as_str()), ("rows", rows_s.as_str())]))
            .set_level(MessageLevel::Info)
            .set_buttons(MessageButtons::YesNo)
            .show(),
    );
    if answer != MessageDialogResult::Yes {
        let _ = proxy.send_event(UiCommand::SetSprite(original));
        return;
    }

    match crate::skin::save_local(slug, &bytes).and_then(|_| crate::config::save_pet_skin(slug)) {
        Ok(()) => {
            let text = fill(&msgs.text("import.saved"), &[("name", file_name.as_str())]);
            if !text.is_empty() { let _ = proxy.send_event(UiCommand::ShowBubble { text, duration_ms: 4000 }); }
        }
        Err(e) => {
            let _ = proxy.send_event(UiCommand::SetSprite(original));
            let err = e.to_string();
            alert(fill(&msgs.text("import.save_error"), &[("err", err.as_str())]));
        }
    }
}

/// 依 .env 的 PET_SKIN 取得外觀：(圖片, 提示氣泡, 需要從 Petdex 下載的名稱)
/// 讀不到時先回傳內建預設圖片
fn load_skin_from_env(msgs: &Messages) -> (Vec<u8>, Option<String>, Option<String>) {
    let default = || crate::skin::DEFAULT_SPRITESHEET.to_vec();
    match crate::config::skin_source(&crate::config::load_env()) {
        SkinSource::Default => (default(), None, None),
        SkinSource::File(path) => match std::fs::read(&path) {
            Ok(b) if crate::skin::is_image(&b) => (b, None, None),
            _ => {
                let name = path.display().to_string();
                (default(), Some(fill(&msgs.text("skin.image_error"), &[("name", name.as_str())])), None)
            }
        },
        SkinSource::Name(name) => match crate::skin::load_local(&name) {
            Some(b) => (b, None, None),
            None => (default(), None, Some(name)),
        },
    }
}

/// 從 Petdex 下載外觀並換上（在背景執行緒呼叫）
fn download_skin(proxy: &tao::event_loop::EventLoopProxy<UiCommand>, msgs: &Messages, name: &str) {
    let bubble = |text: String, ms: u64| {
        if !text.is_empty() { let _ = proxy.send_event(UiCommand::ShowBubble { text, duration_ms: ms }); }
    };
    bubble(fill(&msgs.text("skin.downloading"), &[("name", name)]), 5000);
    match crate::skin::download(name) {
        Ok((_, bytes)) => {
            let _ = proxy.send_event(UiCommand::SetSprite(bytes));
            bubble(msgs.text("skin.installed"), 3000);
        }
        Err(crate::skin::DownloadError::NotFound) => bubble(fill(&msgs.text("skin.not_found"), &[("name", name)]), 4000),
        Err(crate::skin::DownloadError::Network) => bubble(msgs.text("skin.network_error"), 4000),
    }
}

/// 依作用中視窗計算寵物（不含泡泡）左上角，見 `bubble::home_spot`
fn home_spot_for(w: &tao::window::Window, active: Option<crate::bubble::Rect>, base: (i32, i32)) -> (i32, i32) {
    let rect = |m: &tao::monitor::MonitorHandle| {
        let (p, s) = (m.position(), m.size());
        crate::bubble::Rect { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 }
    };
    let monitors: Vec<_> = w.available_monitors().map(|m| rect(&m)).collect();
    let primary = w.primary_monitor().map(|m| rect(&m))
        .or_else(|| monitors.first().copied())
        .unwrap_or(crate::bubble::Rect { x: 0, y: 0, w: 1920, h: 1080 });
    crate::bubble::home_spot(active, &monitors, primary, base)
}

#[cfg(target_os = "windows")]
mod win_active {
    extern "system" {
        pub fn GetForegroundWindow() -> isize;
        pub fn GetAncestor(hwnd: isize, flags: u32) -> isize;
        pub fn IsWindow(hwnd: isize) -> i32;
        pub fn IsWindowVisible(hwnd: isize) -> i32;
        pub fn IsIconic(hwnd: isize) -> i32;
        pub fn GetWindowRect(hwnd: isize, rect: *mut [i32; 4]) -> i32;
        pub fn GetClassNameW(hwnd: isize, buf: *mut u16, len: i32) -> i32;
    }
}

/// 可以當「回家」參考的視窗：可見、沒縮小、不是桌寵自己、不是桌面或工作列
#[cfg(target_os = "windows")]
fn usable_window_rect(hwnd: isize) -> Option<crate::bubble::Rect> {
    use win_active::*;
    if hwnd == 0 { return None; }
    let hwnd = unsafe { GetAncestor(hwnd, 2) }; // GA_ROOT
    if hwnd == 0 || hwnd == PET_HWND.load(std::sync::atomic::Ordering::SeqCst) { return None; }
    unsafe {
        if IsWindow(hwnd) == 0 || IsWindowVisible(hwnd) == 0 || IsIconic(hwnd) != 0 { return None; }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
        let class = String::from_utf16_lossy(&buf[..n]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") { return None; }
        let mut r = [0i32; 4];
        if GetWindowRect(hwnd, &mut r) == 0 { return None; }
        let rect = crate::bubble::Rect { x: r[0], y: r[1], w: r[2] - r[0], h: r[3] - r[1] };
        (rect.w > 0 && rect.h > 0).then_some(rect)
    }
}

/// 作用中視窗的位置
#[cfg(target_os = "windows")]
fn active_window_rect() -> Option<crate::bubble::Rect> {
    usable_window_rect(unsafe { win_active::GetForegroundWindow() })
}

#[cfg(not(target_os = "windows"))]
fn active_window_rect() -> Option<crate::bubble::Rect> { None }

/// 點 (x, y)（實體像素）是否落在任一個已連接的螢幕上
fn on_any_monitor(w: &tao::window::Window, x: i32, y: i32) -> bool {
    w.available_monitors().any(|m| {
        let (p, s) = (m.position(), m.size());
        x >= p.x && y >= p.y && x < p.x + s.width as i32 && y < p.y + s.height as i32
    })
}

/// 內容區在螢幕上的位置與大小（實體像素）
fn content_rect(w: &tao::window::Window) -> crate::bubble::Rect {
    let pos = w.inner_position().or_else(|_| w.outer_position()).unwrap_or_default();
    let size = w.inner_size();
    crate::bubble::Rect { x: pos.x, y: pos.y, w: size.width as i32, h: size.height as i32 }
}

/// 內容區左上角相對外框左上角的位移（看不見的邊框寬度）
fn frame_offset(w: &tao::window::Window) -> (i32, i32) {
    match (w.inner_position(), w.outer_position()) {
        (Ok(i), Ok(o)) => (i.x - o.x, i.y - o.y),
        _ => (0, 0),
    }
}

fn data_dir() -> std::path::PathBuf {
    let home = crate::home_dir().unwrap_or_else(|| ".".into());
    crate::app_dir_in(std::path::Path::new(&home)).join("data")
}

fn save_scale(s: f32) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("pet-scale"), s.to_string());
}

fn save_position(x: i32, y: i32) {
    let dir = data_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("position"), format!("{}\n{}", x, y));
}

fn load_position() -> Option<(i32, i32)> {
    let data = std::fs::read_to_string(data_dir().join("position")).ok()?;
    let mut lines = data.trim().lines();
    let x: i32 = lines.next()?.parse().ok()?;
    let y: i32 = lines.next()?.parse().ok()?;
    Some((x, y))
}