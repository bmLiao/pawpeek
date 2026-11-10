//! 使用者設定：一律放在「文件」資料夾下的 `.pawpeek/`
//!
//! - `messages.json`：所有提示文字（首次啟動自動產生範本，可手改，下次啟動讀入）
//! - `.env`：`PET_SKIN` 指定寵物外觀（Petdex 名稱或本機圖片）

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

pub const DEFAULT_MESSAGES: &str = include_str!("../assets/messages.json");
pub const DEFAULT_ENV: &str = include_str!("../assets/default.env");

/// `<文件>/.pawpeek`
pub fn app_dir() -> PathBuf {
    let docs = documents_dir()
        .or_else(|| crate::home_dir().map(|h| PathBuf::from(h).join("Documents")))
        .unwrap_or_else(|| PathBuf::from("."));
    crate::app_dir_in(&docs)
}

/// Windows：用 SHGetKnownFolderPath 取得真正的「文件」位置（支援 OneDrive 重新導向）
#[cfg(target_os = "windows")]
fn documents_dir() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;

    #[repr(C)]
    struct Guid { d1: u32, d2: u16, d3: u16, d4: [u8; 8] }
    // FOLDERID_Documents {FDD39AD0-238F-46AF-ADB4-6C85480369C7}
    const FOLDERID_DOCUMENTS: Guid = Guid {
        d1: 0xFDD3_9AD0, d2: 0x238F, d3: 0x46AF,
        d4: [0xAD, 0xB4, 0x6C, 0x85, 0x48, 0x03, 0x69, 0xC7],
    };
    extern "system" {
        fn SHGetKnownFolderPath(rfid: *const Guid, flags: u32, token: isize, path: *mut *mut u16) -> i32;
        fn CoTaskMemFree(pv: *mut std::ffi::c_void);
    }
    unsafe {
        let mut p: *mut u16 = std::ptr::null_mut();
        let hr = SHGetKnownFolderPath(&FOLDERID_DOCUMENTS, 0, 0, &mut p);
        let result = if hr == 0 && !p.is_null() {
            let mut len = 0usize;
            while *p.add(len) != 0 { len += 1; }
            let wide = std::slice::from_raw_parts(p, len);
            Some(PathBuf::from(std::ffi::OsString::from_wide(wide)))
        } else {
            None
        };
        if !p.is_null() { CoTaskMemFree(p as *mut std::ffi::c_void); }
        result
    }
}

/// macOS / 其他：使用 ~/Documents
#[cfg(not(target_os = "windows"))]
fn documents_dir() -> Option<PathBuf> { None }

/// 首次啟動：建立資料夾與範本檔（已存在就不覆蓋）
pub fn ensure_files() {
    let dir = app_dir();
    let _ = std::fs::create_dir_all(&dir);
    let messages = dir.join("messages.json");
    if !messages.exists() { let _ = std::fs::write(&messages, DEFAULT_MESSAGES); }
    let env = dir.join(".env");
    if !env.exists() { let _ = std::fs::write(&env, DEFAULT_ENV); }
}

// ---------------------------------------------------------------------------
// messages.json
// ---------------------------------------------------------------------------

/// 提示文字：內建預設值 + 使用者 messages.json 覆蓋（缺的欄位用預設補）
#[derive(Debug, Clone)]
pub struct Messages {
    root: Value,
}

impl Messages {
    pub fn load() -> Self {
        let mut root: Value = serde_json::from_str(DEFAULT_MESSAGES).unwrap_or(Value::Null);
        let path = app_dir().join("messages.json");
        if let Ok(s) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Value>(s.trim_start_matches('\u{feff}')) {
                Ok(user) => merge(&mut root, user),
                Err(e) => eprintln!("[pawpeek] messages.json 格式錯誤，改用預設值: {e}"),
            }
        }
        Self { root }
    }

    /// 給前端用的 JSON（已處理 `</script>` 跳脫）
    pub fn to_js(&self) -> String {
        serde_json::to_string(&self.root).unwrap_or_else(|_| "{}".into()).replace("</", "<\\/")
    }

    fn get(&self, path: &str) -> Option<&Value> {
        path.split('.').try_fold(&self.root, |v, k| v.get(k))
    }

    /// 取單一字串（若該欄位是陣列則隨機挑一句）
    pub fn text(&self, path: &str) -> String {
        match self.get(path) {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(_)) => self.pick(path),
            _ => String::new(),
        }
    }

    /// 從陣列隨機挑一句（若該欄位是字串則直接回傳）
    pub fn pick(&self, path: &str) -> String {
        match self.get(path) {
            Some(Value::Array(a)) => {
                let items: Vec<&str> = a.iter().filter_map(|v| v.as_str()).collect();
                if items.is_empty() { String::new() } else { items[rand_index(items.len())].to_string() }
            }
            Some(Value::String(s)) => s.clone(),
            _ => String::new(),
        }
    }
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(bv) => merge(bv, v),
                    None => { b.insert(k, v); }
                }
            }
        }
        (b, o) => *b = o,
    }
}

/// 把 `{key}` 換成對應值
pub fn fill(template: &str, vars: &[(&str, &str)]) -> String {
    let mut s = template.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{}}}", k), v);
    }
    s
}

pub fn rand_index(n: usize) -> usize {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
    );
    (h.finish() as usize) % n.max(1)
}

// ---------------------------------------------------------------------------
// .env
// ---------------------------------------------------------------------------

pub fn load_env() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let content = std::fs::read_to_string(app_dir().join(".env")).unwrap_or_default();
    for line in content.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim();
        let v = if v.len() >= 2
            && ((v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')))
        {
            &v[1..v.len() - 1]
        } else {
            // 未加引號時允許行尾註解：PET_SKIN=boba  # 註解
            v.split(" #").next().unwrap_or("").trim()
        };
        map.insert(k.trim().to_string(), v.to_string());
    }
    map
}

/// 改寫 .env 內容中的 `key=value`（保留註解與其他行；找不到就附加在最後）
pub fn set_env_value(content: &str, key: &str, value: &str) -> String {
    let nl = if content.contains("\r\n") { "\r\n" } else { "\n" };
    let is_key_line = |line: &str| {
        let t = line.trim_start_matches('\u{feff}').trim();
        let t = t.strip_prefix("export ").unwrap_or(t);
        !t.starts_with('#') && t.split_once('=').map(|(k, _)| k.trim() == key).unwrap_or(false)
    };
    let mut found = false;
    let mut lines: Vec<String> = content
        .lines()
        .map(|line| {
            if !found && is_key_line(line) {
                found = true;
                format!("{key}={value}")
            } else {
                line.to_string()
            }
        })
        .collect();
    if !found { lines.push(format!("{key}={value}")); }
    lines.join(nl) + nl
}

/// 把 PET_SKIN 寫入 `<文件>/.pawpeek/.env`
pub fn save_pet_skin(name: &str) -> std::io::Result<()> {
    let path = app_dir().join(".env");
    let content = std::fs::read_to_string(&path).unwrap_or_else(|_| DEFAULT_ENV.to_string());
    std::fs::write(&path, set_env_value(&content, "PET_SKIN", name))
}

/// PET_SKIN 的解析結果
#[derive(Debug, Clone)]
pub enum SkinSource {
    /// 留空：內建預設寵物
    Default,
    /// 本機圖片路徑
    File(PathBuf),
    /// Petdex 名稱
    Name(String),
}

pub fn skin_source(env: &HashMap<String, String>) -> SkinSource {
    parse_skin(env.get("PET_SKIN").map(|s| s.as_str()).unwrap_or(""))
}

/// 解析 PET_SKIN 的值（也用於 `--set-skin` 的參數）
pub fn parse_skin(v: &str) -> SkinSource {
    let v = v.trim();
    if v.is_empty() {
        return SkinSource::Default;
    }
    let lower = v.to_lowercase();
    let looks_like_file = lower.ends_with(".png") || lower.ends_with(".webp")
        || v.contains('/') || v.contains('\\');
    if looks_like_file {
        let p = PathBuf::from(v);
        let p = if p.is_absolute() { p } else { app_dir().join(p) };
        SkinSource::File(p)
    } else {
        SkinSource::Name(v.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_env_replaces_existing_line() {
        let src = "# 註解 PET_SKIN=xx\nPET_SKIN=mallow\nOTHER=1\n";
        assert_eq!(set_env_value(src, "PET_SKIN", "machi"), "# 註解 PET_SKIN=xx\nPET_SKIN=machi\nOTHER=1\n");
    }

    #[test]
    fn set_env_replaces_export_and_spaced() {
        assert_eq!(set_env_value("export PET_SKIN = a\n", "PET_SKIN", "b"), "PET_SKIN=b\n");
    }

    #[test]
    fn set_env_appends_when_missing() {
        assert_eq!(set_env_value("# x\nOTHER=1", "PET_SKIN", "b"), "# x\nOTHER=1\nPET_SKIN=b\n");
        assert_eq!(set_env_value("", "PET_SKIN", "b"), "PET_SKIN=b\n");
    }

    #[test]
    fn set_env_keeps_crlf() {
        assert_eq!(set_env_value("A=1\r\nPET_SKIN=a\r\n", "PET_SKIN", "b"), "A=1\r\nPET_SKIN=b\r\n");
    }
}
