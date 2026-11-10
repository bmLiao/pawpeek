# PawPeek — 開發與更新指南

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

## 更新已安裝的插件

改完程式碼後依序執行：

1. **調高版本號**：修改 `.claude-plugin/plugin.json` 的 `version`（例如 1.2.0 → 1.2.1）。
   Claude Code 以版本號判斷是否更新，版本不變會沿用舊快取。
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

## 故障排除

| 問題 | 處理 |
|------|------|
| 桌寵不顯示 | `Stop-Process -Name pawpeek` 後開新的 Claude Code 對話 |
| 通訊埠 47390 被佔用 | 結束佔用的程式，或改 `src/daemon.rs` 的 `FIXED_PORT` 後重新編譯 |
| 外觀圖片不顯示 | 確認 `PET_SKIN` 指向的 spritesheet 存在，且為 8 欄 × 9 或 11 列 |
