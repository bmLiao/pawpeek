# PawPeek — 開發與更新指南

> 🤖 **AI 開發專案**：程式碼與文件由 AI（Anthropic Claude，透過 Claude Code）撰寫，想法與需求來自 BM.Liao。
>
> PawPeek fork 自 [Jedeiah/agent-critter](https://github.com/Jedeiah/agent-critter)（MIT）。使用說明見 [README.md](README.md)。

## 第一次安裝

```powershell
.\scripts\build-plugin.bat
```

```
/plugin marketplace add C:\path\to\pawpeek\dist\pawpeek-plugin-windows
/plugin install pawpeek@pawpeek
/reload-plugins
```

本機安裝是把 `dist\pawpeek-plugin-windows` 加成 directory marketplace，
Claude Code 會把它複製到 `~/.claude/plugins/cache/pawpeek/pawpeek/<版本>/`。

## repo 的 bin/ 結構（從 GitHub 安裝用）

| 檔案 | 說明 |
|------|------|
| `bin/pawpeek` | sh 啟動器，依 `uname` 轉交給下面兩個執行檔；hooks 與斜線指令都呼叫它 |
| `bin/pawpeek.exe` | Windows 執行檔（CI 編譯後自動 commit） |
| `bin/pawpeek-macos` | macOS arm64 執行檔（CI 編譯後自動 commit） |
| `bin/VERSION` | 已部署執行檔的版本號（CI 自動寫入）；桌寵啟動時讀一次，比自己新就提醒使用者更新 |

注意：在 Git Bash 裡 `bin/pawpeek` 會被當成 `bin/pawpeek.exe`，用 shell 寫入 `bin/pawpeek` 會覆蓋掉 exe，要改啟動器請用編輯器。
`dist\` 打包出來的資料夾只放單一平台執行檔，不需要啟動器。

## 更新已安裝的插件

改完程式碼後依序執行：

1. **調高版本號**：`.claude-plugin/plugin.json` 與 `Cargo.toml` 的 `version` **兩個都要改、而且要一致**（例如 1.2.0 → 1.2.1）。
   Claude Code 以 plugin.json 判斷是否更新（版本不變會沿用舊快取）；執行檔以 Cargo.toml 判斷要不要提醒使用者更新。
   兩邊不一致時 CI 的 `check-version` 會失敗、不會部署。
2. **編譯**：
   ```powershell
   cargo build --release
   ```
3. **關掉桌寵後立刻打包**：桌寵從 `dist\...\bin\pawpeek.exe` 執行，它和 WebView2 子程序
   （資料夾 `bin\pawpeek.exe.WebView2`）會鎖住檔案，打包腳本會報 `Failed to copy binary`。
   其他開著的 Claude Code session 的 hook 會隨時把桌寵重新叫起來，所以「關閉」和「打包」要寫在同一個指令裡：
   ```powershell
   Get-CimInstance Win32_Process |
     ? { $_.Name -eq 'pawpeek.exe' -or ($_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -like '*pawpeek*') } |
     % { Stop-Process -Id $_.ProcessId -Force }
   Start-Sleep -Milliseconds 800
   cmd /c "scripts\build-plugin.bat windows"
   ```
   仍失敗就再執行一次（第一次通常已清掉被鎖的舊檔）。
4. **更新 marketplace 與插件**（在 Claude Code 內）：
   ```
   /plugin marketplace update pawpeek
   /plugin update pawpeek@pawpeek
   /reload-plugins
   ```
   或用 CLI（若目前 shell 設了 `CLAUDE_CONFIG_DIR` 指向別的設定檔，要先清掉，才會作用在 `~/.claude`）：
   ```powershell
   Remove-Item Env:CLAUDE_CONFIG_DIR -ErrorAction SilentlyContinue
   claude plugin marketplace update pawpeek
   claude plugin update pawpeek@pawpeek
   ```
   仍是舊版時，改用 `/plugin uninstall pawpeek@pawpeek` 再 `/plugin install pawpeek@pawpeek`。
5. 開新的 session，SessionStart hook 會啟動新版桌寵。

## 新版提醒的運作方式

推送到 main → CI 編譯、測試 → `deploy` 把兩個執行檔與 `bin/VERSION` 一起 commit。
桌寵啟動 5 秒後讀一次 GitHub 上的 `bin/VERSION`（之後不再檢查），比自己的版本新就把提醒暫存起來，
等第一次待機時顯示成「一次性持久泡泡」（`setBubble(text, 0, true, true)`）：一直留著，
直到狀態改變（例如切到工作中）或被其他泡泡蓋過（點擊、拖曳、打招呼等）就結束，這次執行不再出現。
顯示期間閒置動作只播動畫、不冒泡泡（看 `window.__onceBubble`），閒置動作不算打斷提醒。
提醒文字在 `messages.json` 的 `update.available`。

`bin/VERSION` 只在執行檔部署後才更新，所以提醒出現時新版執行檔一定已經在 repo 裡。
但沒透過提醒、在 CI 跑完前就手動更新的使用者，仍可能拿到「新版本號、舊執行檔」，且之後因版本號相同不會再更新；
這時請他們 `/plugin uninstall pawpeek@pawpeek` 再重新安裝。

## 動作與說話規格

寵物什麼時候做什麼動作、說什麼話，完整規格在 [docs/BEHAVIOR.md](docs/BEHAVIOR.md)。
改到狀態、動畫、泡泡、台詞或互動相關的程式前先讀它，改完一併更新。

## 故障排除

| 問題 | 處理 |
|------|------|
| 桌寵不顯示 | `Stop-Process -Name pawpeek` 後開新的 Claude Code 對話 |
| 通訊埠 47390 被佔用 | 結束佔用的程式，或改 `src/daemon.rs` 的 `FIXED_PORT` 後重新編譯 |
| 外觀圖片不顯示 | 確認 `PET_SKIN` 指向的 spritesheet 存在，且為 8 欄 × 9 或 11 列 |
