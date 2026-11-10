# PawPeek 開發須知

## 動作與說話規格

寵物的狀態、動畫、泡泡、台詞與互動行為以 [`docs/BEHAVIOR.md`](docs/BEHAVIOR.md) 為唯一規格。

- **修改前先讀**：只要改動會影響下列任何一項，動手前先讀 `docs/BEHAVIOR.md`，確認現行規則：
  - 狀態或 hook 對應（`src/hook.rs`、`src/state.rs`、`src/daemon.rs` 的狀態通知）
  - 動畫、泡泡顯示與說話時機（`src/bubble.rs`、`src/webview.rs` 的 `CONTROLLER_JS` / `STATES` / `play`）
  - 拖曳、單擊、雙擊、縮放、自言自語、打招呼 / 告別、更新提醒、外觀提示
  - 台詞欄位（`assets/messages.json`）
- **改完要同步**：行為有變動就更新 `docs/BEHAVIOR.md` 對應章節，並在「變更紀錄」加一行。
- **衝突時先問**：使用者的新要求與文件規則衝突時，先指出衝突並確認，再同時改程式與文件。

## 其他

- 開發、打包、更新已安裝插件的流程見 [`PLUGIN.md`](PLUGIN.md)。
- 版本號：`Cargo.toml` 與 `.claude-plugin/plugin.json` 要一起改且一致（CI 會檢查）。
- 介面文字與文件使用繁體中文。
