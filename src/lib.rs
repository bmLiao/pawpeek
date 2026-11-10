#![allow(unexpected_cfgs)]

#[cfg(target_os = "macos")]
#[macro_use] extern crate objc;

pub mod assets;
pub mod bubble;
pub mod client;
pub mod config;
pub mod daemon;
pub mod hook;
pub mod server;
pub mod skin;
pub mod state;
pub mod update;
pub mod webview;

/// 使用者資料夾名稱（放在「文件」與家目錄下）
pub const APP_DIR_NAME: &str = ".pawpeek";
/// 改名前（fork 自 agent-critter）用的資料夾名稱
const LEGACY_APP_DIR_NAME: &str = ".agent-critter";

/// `<parent>/.pawpeek`；新資料夾還不存在但有舊的 `.agent-critter` 時，直接搬過來沿用
pub fn app_dir_in(parent: &std::path::Path) -> std::path::PathBuf {
    let dir = parent.join(APP_DIR_NAME);
    if !dir.exists() {
        let legacy = parent.join(LEGACY_APP_DIR_NAME);
        if legacy.is_dir() {
            let _ = std::fs::rename(&legacy, &dir);
        }
    }
    dir
}

pub fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}
