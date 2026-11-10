//! 寵物外觀（spritesheet）載入與 Petdex 下載

use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;

pub const DEFAULT_SPRITESHEET: &[u8] = include_bytes!("../assets/default_spritesheet.webp");

/// 本機寵物資料夾（依序搜尋）：
/// `<文件>/.pawpeek/pets`、`~/.codex/pets`、`~/.petdex/pets`
fn pet_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![crate::config::app_dir().join("pets")];
    if let Some(h) = crate::home_dir() {
        let h = PathBuf::from(h);
        dirs.push(h.join(".codex").join("pets"));
        dirs.push(h.join(".petdex").join("pets"));
    }
    dirs
}

fn safe_slug(s: &str) -> bool {
    !s.is_empty() && !s.contains("..") && !s.contains('/') && !s.contains('\\')
}

/// 名稱正規化：小寫、空白轉 -
pub fn normalize(name: &str) -> String {
    name.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
}

/// 是否為可用圖片（png / webp）
pub fn is_image(b: &[u8]) -> bool {
    b.starts_with(b"\x89PNG") || (b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP")
}

pub fn mime(b: &[u8]) -> &'static str {
    if b.len() >= 12 && &b[0..4] == b"RIFF" && &b[8..12] == b"WEBP" { "image/webp" } else { "image/png" }
}

/// 讀出 png / webp 的寬高（只看檔頭）
pub fn image_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.starts_with(b"\x89PNG") {
        if b.len() < 24 || &b[12..16] != b"IHDR" { return None; }
        let w = u32::from_be_bytes(b[16..20].try_into().ok()?);
        let h = u32::from_be_bytes(b[20..24].try_into().ok()?);
        return Some((w, h));
    }
    if b.len() < 30 || &b[0..4] != b"RIFF" || &b[8..12] != b"WEBP" { return None; }
    let le24 = |s: &[u8]| s[0] as u32 | (s[1] as u32) << 8 | (s[2] as u32) << 16;
    match &b[12..16] {
        b"VP8X" => Some((le24(&b[24..27]) + 1, le24(&b[27..30]) + 1)),
        b"VP8L" => {
            let v = u32::from_le_bytes(b[21..25].try_into().ok()?);
            Some(((v & 0x3fff) + 1, ((v >> 14) & 0x3fff) + 1))
        }
        b"VP8 " => {
            let w = u16::from_le_bytes([b[26], b[27]]) as u32 & 0x3fff;
            let h = u16::from_le_bytes([b[28], b[29]]) as u32 & 0x3fff;
            Some((w, h))
        }
        _ => None,
    }
}

/// 依尺寸算列數：8 欄、每格 192×208（可等比例縮放），只接受 9 或 11 列
/// （判斷方式與 webview.rs 的 loadSprite 一致）
pub fn sheet_rows(w: u32, h: u32) -> Option<u32> {
    if w == 0 || h == 0 { return None; }
    let cell_h = w as f64 / 8.0 * 208.0 / 192.0;
    let rows = h as f64 / cell_h;
    let n = rows.round();
    if (rows - n).abs() > 0.01 { return None; }
    match n as u32 { n @ (9 | 11) => Some(n), _ => None }
}

#[derive(Debug)]
pub enum SheetError {
    NotImage,
    BadSize { w: u32, h: u32 },
}

/// 檢查是否為可用的 spritesheet，回傳列數
pub fn validate_sheet(b: &[u8]) -> Result<u32, SheetError> {
    if !is_image(b) { return Err(SheetError::NotImage); }
    let (w, h) = image_size(b).ok_or(SheetError::NotImage)?;
    sheet_rows(w, h).ok_or(SheetError::BadSize { w, h })
}

/// 自選圖片固定存成這個寵物名稱（只保留最新一張）
pub const CUSTOM_SLUG: &str = "custom";

/// 存到 `<文件>/.pawpeek/pets/<slug>/spritesheet.<ext>`（先刪掉舊的 png/webp，避免讀到舊檔）
pub fn save_local(slug: &str, img: &[u8]) -> std::io::Result<PathBuf> {
    if !safe_slug(slug) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid name"));
    }
    let dir = crate::config::app_dir().join("pets").join(slug);
    std::fs::create_dir_all(&dir)?;
    for ext in ["webp", "png"] {
        let _ = std::fs::remove_file(dir.join(format!("spritesheet.{ext}")));
    }
    let ext = if mime(img) == "image/webp" { "webp" } else { "png" };
    let path = dir.join(format!("spritesheet.{ext}"));
    std::fs::write(&path, img)?;
    Ok(path)
}

/// 從本機寵物資料夾讀取 spritesheet
pub fn load_local(name: &str) -> Option<Vec<u8>> {
    let mut candidates = vec![name.trim().to_string()];
    let n = normalize(name);
    if !candidates.contains(&n) { candidates.push(n); }
    for slug in candidates.iter().filter(|s| safe_slug(s)) {
        for base in pet_dirs() {
            for ext in ["webp", "png"] {
                if let Ok(b) = std::fs::read(base.join(slug).join(format!("spritesheet.{ext}"))) {
                    if is_image(&b) { return Some(b); }
                }
            }
        }
    }
    None
}

#[derive(Debug)]
pub enum DownloadError {
    /// 網路或伺服器問題
    Network,
    /// Petdex 上找不到這個名稱
    NotFound,
}

/// 從 Petdex 下載並存到 `<文件>/.pawpeek/pets/<slug>/`，回傳 (slug, 圖片)
pub fn download(name: &str) -> Result<(String, Vec<u8>), DownloadError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into();

    let resp = agent.get("https://petdex.dev/api/manifest").call().map_err(|e| {
        eprintln!("[pawpeek] manifest 請求失敗: {e:?}");
        DownloadError::Network
    })?;
    let mut raw = Vec::new();
    resp.into_body().into_reader().read_to_end(&mut raw).map_err(|_| DownloadError::Network)?;
    let root: serde_json::Value = serde_json::from_slice(&raw).map_err(|e| {
        eprintln!("[pawpeek] manifest 解析失敗: {e:?}");
        DownloadError::Network
    })?;
    let entries = root.get("pets").and_then(|p| p.as_array()).ok_or(DownloadError::Network)?;

    let want = normalize(name);
    let field = |e: &serde_json::Value, k: &str| e.get(k).and_then(|v| v.as_str()).map(normalize);
    let entry = entries
        .iter()
        .find(|e| field(e, "slug").as_deref() == Some(want.as_str()) || field(e, "displayName").as_deref() == Some(want.as_str()))
        .ok_or(DownloadError::NotFound)?;

    let slug = entry.get("slug").and_then(|v| v.as_str()).map(str::to_string).unwrap_or(want);
    if !safe_slug(&slug) { return Err(DownloadError::NotFound); }
    let url = entry.get("spritesheetUrl").and_then(|u| u.as_str()).ok_or(DownloadError::NotFound)?;

    let img_resp = agent.get(url).call().map_err(|e| {
        eprintln!("[pawpeek] 精靈圖下載失敗: {e:?}");
        DownloadError::Network
    })?;
    let mut img = Vec::new();
    img_resp.into_body().into_reader().read_to_end(&mut img).map_err(|_| DownloadError::Network)?;
    if !is_image(&img) { return Err(DownloadError::Network); }

    let dir = crate::config::app_dir().join("pets").join(&slug);
    let ext = if mime(&img) == "image/webp" { "webp" } else { "png" };
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::write(dir.join(format!("spritesheet.{ext}")), &img);
    }
    Ok((slug, img))
}

/// 本機已有的寵物名稱（`<文件>/.pawpeek/pets` 等資料夾內有 spritesheet 的子資料夾）
pub fn list_local() -> Vec<String> {
    let mut names = Vec::new();
    for base in pet_dirs() {
        let Ok(rd) = std::fs::read_dir(&base) else { continue };
        for e in rd.flatten() {
            let has_sheet = ["webp", "png"].iter().any(|ext| e.path().join(format!("spritesheet.{ext}")).is_file());
            let name = e.file_name().to_string_lossy().into_owned();
            if has_sheet && !names.contains(&name) { names.push(name); }
        }
    }
    names.sort();
    names
}

/// 「預設」的各種寫法
pub fn is_default_keyword(v: &str) -> bool {
    matches!(v.trim().to_lowercase().as_str(), "default" | "預設" | "预设" | "reset")
}

fn sheet_error_text(e: SheetError, name: &str) -> String {
    match e {
        SheetError::NotImage => format!("「{name}」不是 png 或 webp 圖片"),
        SheetError::BadSize { w, h } => format!(
            "「{name}」尺寸 {w}×{h} 不符合規定（需要 8 欄、9 或 11 列，每格 192×208，可等比例縮放）"
        ),
    }
}

/// `--set-skin <值>`：解析參數、準備好圖片並寫入 .env。
/// 成功回傳給使用者看的說明，失敗回傳錯誤原因（.env 不會被修改）。
pub fn apply(value: &str) -> Result<String, String> {
    use crate::config::{parse_skin, save_pet_skin, SkinSource};
    let value = value.trim();
    let save = |v: &str| save_pet_skin(v).map_err(|e| format!("寫入 .env 失敗：{e}"));

    if is_default_keyword(value) {
        save("")?;
        return Ok("已換回內建預設外觀。".into());
    }
    match parse_skin(value) {
        SkinSource::Default => Err("請提供寵物名稱或圖片路徑。".into()),
        SkinSource::File(path) => {
            // 指令列的相對路徑先以目前目錄為準，找不到才用設定資料夾
            let path = match std::env::current_dir() {
                Ok(cwd) if std::path::Path::new(value).is_relative() && cwd.join(value).is_file() => cwd.join(value),
                _ => path,
            };
            let shown = path.display().to_string();
            let bytes = std::fs::read(&path).map_err(|e| format!("讀不到圖片「{shown}」：{e}"))?;
            let rows = validate_sheet(&bytes).map_err(|e| sheet_error_text(e, &shown))?;
            save_local(CUSTOM_SLUG, &bytes).map_err(|e| format!("存檔失敗：{e}"))?;
            save(CUSTOM_SLUG)?;
            Ok(format!("已換成自訂圖片「{shown}」（{rows} 列，存為 {CUSTOM_SLUG}）。"))
        }
        SkinSource::Name(name) => {
            let (slug, bytes, downloaded) = match load_local(&name) {
                Some(b) => (name.clone(), b, false),
                None => {
                    println!("本機沒有「{name}」，從 Petdex 下載中……");
                    match download(&name) {
                        Ok((slug, b)) => (slug, b, true),
                        Err(DownloadError::NotFound) => return Err(format!("Petdex 上找不到「{name}」。")),
                        Err(DownloadError::Network) => return Err("網路錯誤，無法從 Petdex 下載。".into()),
                    }
                }
            };
            validate_sheet(&bytes).map_err(|e| sheet_error_text(e, &slug))?;
            save(&slug)?;
            Ok(if downloaded { format!("已下載並換成「{slug}」。") } else { format!("已換成「{slug}」。") })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        b.extend_from_slice(&w.to_be_bytes());
        b.extend_from_slice(&h.to_be_bytes());
        b.extend_from_slice(&[8, 6, 0, 0, 0]);
        b
    }

    fn webp_vp8l(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
        let v = (w - 1) | ((h - 1) << 14);
        b.extend_from_slice(&v.to_le_bytes());
        b.extend_from_slice(&[0; 8]);
        b
    }

    #[test]
    fn reads_png_size() {
        assert_eq!(image_size(&png(1536, 1872)), Some((1536, 1872)));
    }

    #[test]
    fn reads_webp_size() {
        assert_eq!(image_size(&webp_vp8l(1536, 2288)), Some((1536, 2288)));
    }

    #[test]
    fn rejects_unknown_format() {
        assert_eq!(image_size(b"GIF89a....................."), None);
    }

    #[test]
    fn rows_from_size() {
        assert_eq!(sheet_rows(1536, 1872), Some(9));
        assert_eq!(sheet_rows(1536, 2288), Some(11));
        assert_eq!(sheet_rows(768, 936), Some(9)); // 等比例縮小
        assert_eq!(sheet_rows(1536, 2080), None); // 10 列
        assert_eq!(sheet_rows(1500, 1872), None); // 比例不對
        assert_eq!(sheet_rows(0, 0), None);
    }

    #[test]
    fn validate_sheet_errors() {
        assert!(matches!(validate_sheet(b"nope"), Err(SheetError::NotImage)));
        assert!(matches!(validate_sheet(&png(1536, 2080)), Err(SheetError::BadSize { w: 1536, h: 2080 })));
        assert_eq!(validate_sheet(&png(1536, 2288)).unwrap(), 11);
    }
}
