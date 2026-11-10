//! 新版檢查：桌寵啟動時去 GitHub 看一次最新版本，比目前新就在第一次待機時冒泡泡提醒
//! （持續到該次待機結束，被其他狀態蓋掉後不再出現）。
//!
//! 版本來源是 CI 部署執行檔後寫入的 `bin/VERSION`（不是 plugin.json），
//! 確保提醒出現時新版執行檔已經在 repo 裡，使用者更新不會拿到「新版本號、舊執行檔」。
//! 可在 `.env` 設 `UPDATE_CHECK=off` 關閉。

use std::io::Read;
use std::time::Duration;

/// 目前執行檔的版本
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

const VERSION_URL: &str = "https://raw.githubusercontent.com/bmLiao/pawpeek/main/bin/VERSION";

/// `.env` 的 `UPDATE_CHECK`：留空或沒設定 = 開啟；off / false / 0 / no = 關閉
pub fn enabled(env: &std::collections::HashMap<String, String>) -> bool {
    !matches!(
        env.get("UPDATE_CHECK").map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("off" | "false" | "0" | "no")
    )
}

/// 把 "1.2.3"、"v1.2.3"、"1.2.3-beta" 解析成 (1, 2, 3)；缺的欄位補 0
pub fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let core = s.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.trim().parse().ok()?;
    let minor = parts.next().map(|p| p.trim().parse()).transpose().ok()?.unwrap_or(0);
    let patch = parts.next().map(|p| p.trim().parse()).transpose().ok()?.unwrap_or(0);
    Some((major, minor, patch))
}

/// `latest` 是否比 `current` 新（任一個解析不了就當作不是）
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

/// 向 GitHub 取最新版本號
fn fetch_latest() -> Option<String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .build()
        .into();
    let resp = agent.get(VERSION_URL).call().ok()?;
    let mut raw = String::new();
    resp.into_body().into_reader().take(4096).read_to_string(&mut raw).ok()?;
    let v = raw.trim().to_string();
    parse_version(&v).map(|_| v)
}

/// 有新版就回傳其版本號；沒有新版或網路失敗回傳 None
pub fn check() -> Option<String> {
    let latest = fetch_latest()?;
    is_newer(&latest, CURRENT_VERSION).then_some(latest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn parses_versions() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version(" v1.2.3\n"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.3"), Some((1, 3, 0)));
        assert_eq!(parse_version("2"), Some((2, 0, 0)));
        assert_eq!(parse_version("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("<html>404</html>"), None);
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("1.2.10", "1.2.9"));
        assert!(is_newer("1.3.0", "1.2.99"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.2.1", "1.2.1"));
        assert!(!is_newer("1.2.0", "1.2.1"));
        assert!(!is_newer("garbage", "1.0.0"));
    }

    #[test]
    fn enabled_by_default_and_can_be_turned_off() {
        let env = |v: &str| HashMap::from([("UPDATE_CHECK".to_string(), v.to_string())]);
        assert!(enabled(&HashMap::new()));
        assert!(enabled(&env("")));
        assert!(enabled(&env("on")));
        for off in ["off", "OFF", "false", "0", "no"] {
            assert!(!enabled(&env(off)), "{off}");
        }
    }

    #[test]
    fn current_version_is_valid() {
        assert!(parse_version(CURRENT_VERSION).is_some());
    }
}
