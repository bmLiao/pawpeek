# PawPeek 🐾

會探頭看你 AI 在忙什麼的桌寵。PawPeek 是 Claude Code 插件，用一隻透明背景、浮在桌面上的小寵物，即時反映 Claude Code 的工作狀態：閒置、工作中、等你確認、出錯。

> **🤖 AI 開發專案**：PawPeek 的程式碼、文件與修改內容由 AI（Anthropic Claude，透過 Claude Code）撰寫完成。
> 想法、需求與測試回饋來自 BM.Liao，實作則是 AI 完成的。使用前請自行評估；如有問題歡迎回報。

> **原出處**：本專案 fork 自 [Jedeiah/agent-critter](https://github.com/Jedeiah/agent-critter)（MIT License），
> 感謝原作者 chj 與 Agent Critter Contributors。PawPeek 在其基礎上改名並大幅修改，
> 原始版權聲明保留於 [LICENSE](LICENSE)。

| 待機 | 工作中 | 放大 |
|------|--------|------|
| ![待機](idle.png) | ![工作中](working.png) | ![放大](zoomed.png) |

## 與原版的差異

| 項目 | PawPeek |
|------|---------|
| 介面語言 | 繁體中文 |
| 對話泡泡 | 寵物頭上的打字泡泡；靠近螢幕上緣時自動改到下方，視窗跟著泡泡伸縮、寵物位置不動 |
| 自訂文字 | 所有提示文字集中在 `messages.json`，可自行修改 |
| 自訂外觀 | `.env` 的 `PET_SKIN` 指定 Petdex 名稱或本機 spritesheet；`/skin` 選檔匯入、`/set-skin` 切換 |
| Petdex 精靈 | 本機沒有時自動從 [petdex.dev](https://petdex.dev) 下載 |
| 焦點 | 啟動時不搶焦點；點寵物才取得焦點（系統預設行為） |
| 置頂 | 只在啟動時置頂一次，不會打斷全螢幕遊戲/影片 |
| 啟動位置 | 沿用上次位置；不在任何螢幕上時重置到作用中視窗旁 |
| 多螢幕 | 跨不同縮放比例的螢幕時自動校正大小與位置 |
| 通訊埠 | 改用 47390（避開常被代理軟體佔用的 7890） |
| 設定位置 | 文件資料夾下 `.pawpeek/`；舊的 `.agent-critter/` 會自動搬過來 |

## 安裝

需求：Windows 10/11（WebView2 Runtime，Win11 內建）或 macOS（Apple Silicon），以及 [Claude Code](https://docs.claude.com/en/docs/claude-code)。

### 方式一：直接從 GitHub 安裝（建議，免 clone、免編譯）

repo 本身就是 Claude Code marketplace，`bin/` 內含 CI 編好的執行檔。在 Claude Code 對話中輸入：

```
/plugin marketplace add bmLiao/pawpeek
/plugin install pawpeek@pawpeek
/reload-plugins
```

或在終端機用 CLI：

```powershell
claude plugin marketplace add bmLiao/pawpeek
claude plugin install pawpeek@pawpeek
```

也可以直接交給 Claude：在 Claude Code 裡說「幫我安裝 https://github.com/bmLiao/pawpeek 這個插件」，它會執行上面的指令。

#### 保持最新版

**建議開啟自動更新**：第三方 marketplace 預設不會自動更新。在 Claude Code 輸入 `/plugin`，
切到 **Marketplaces** 分頁、選 `pawpeek`，再選 **Enable auto-update**。之後送出每個對話的第一則訊息後，
Claude Code 會在 10 分鐘內自動抓新版，看到「Plugin updated」提示時執行 `/reload-plugins` 或重開即可。

沒開自動更新也沒關係：桌寵每次啟動會檢查一次，有新版就在第一次待機時冒泡泡提醒你，留到狀態改變（包括拖曳寵物）為止（可在 `.env` 設 `UPDATE_CHECK=off` 關閉）。
手動更新：

```
/plugin marketplace update pawpeek
/plugin update pawpeek@pawpeek
/reload-plugins
```

> 若環境變數設了 `DISABLE_AUTOUPDATER=1` 或 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`，自動更新會整個停用；
> 想保留插件自動更新可另外設 `FORCE_AUTOUPDATE_PLUGINS=1`。

### 方式二：從原始碼編譯（開發用）

需要另外安裝 [Rust](https://rustup.rs) 與 Git。

```powershell
git clone https://github.com/bmLiao/pawpeek.git
cd pawpeek

# 編譯並打包到 dist\pawpeek-plugin-windows（此資料夾就是本機 marketplace）
.\scripts\build-plugin.bat
```

```
/plugin marketplace add C:\path\to\pawpeek\dist\pawpeek-plugin-windows
/plugin install pawpeek@pawpeek
/reload-plugins
```

改完程式碼後如何更新已安裝的插件，請見 [PLUGIN.md](PLUGIN.md)。

> 兩種方式的 marketplace 名稱都是 `pawpeek`，切換時先 `/plugin marketplace remove pawpeek` 再重新加入。
> 若 shell 設了 `CLAUDE_CONFIG_DIR`，插件會裝到該設定目錄，而不是 `~/.claude`。

### 確認

開一個新的 Claude Code 對話，狀態列出現「桌寵小精靈上線中...」，接著桌寵出現在桌面上即安裝成功。
也可以用 `/plugin` 查看 `pawpeek@pawpeek` 是否已啟用。

## 使用

| 操作 | 效果 |
|------|------|
| 單擊寵物 | 依目前狀態隨機說一句（待機時加一個小動作） |
| 雙擊寵物 | 顯示目前對話數與狀態 |
| 拖曳 | 移動位置；待機時按住就會說話，並依滑鼠左右方向轉身，放開後動作播完才回待機 |
| `[` / `]` | 縮小 / 放大（0.5x ~ 1.5x，點一下寵物後可用） |

### 斜線指令

| 指令 | 說明 |
|------|------|
| `/skin` | 跳出選檔視窗匯入自訂外觀，預覽後確認 |
| `/set-skin [名稱\|路徑\|default]` | 切換外觀；不帶參數會列出可用外觀 |
| `/quit` | 讓桌寵離開（下次開新對話會再出現） |

### 狀態對應

| Claude Code 狀態 | 寵物動作 |
|------------------|----------|
| 閒置 | 呼吸待機，偶爾自言自語 |
| 工作中 | 原地奔跑 |
| 等待確認（權限請求） | 等待 |
| 限流 / 伺服器錯誤 | 檢查（單一工具失敗不算，維持工作中） |
| 嚴重錯誤（認證、帳單、模型） | 崩潰 |

完整的動作與說話規則見 [docs/BEHAVIOR.md](docs/BEHAVIOR.md)。

多個對話同時進行時取最高優先：`等待確認 > 嚴重錯誤 > 工具出錯 > 工作中 > 閒置`（等待確認代表 Claude 正停下來等你，最需要注意）。

某個對話停在工作中、等待確認或工具出錯超過 30 分鐘沒有任何動靜（例如終端機被直接關掉），會自動回到閒置；嚴重錯誤則保留到你下一次操作。

## 設定檔

位於「文件」資料夾下的 `.pawpeek/`（首次啟動自動產生範本）：

| 檔案 | 用途 |
|------|------|
| `messages.json` | 所有提示文字；陣列會隨機挑一句，`{count}`、`{name}`、`{state}` 自動帶入 |
| `.env` | `PET_SKIN=`：Petdex 名稱、本機圖片路徑，或留空用內建寵物；`UPDATE_CHECK=off` 關閉新版提醒 |
| `pets/<名稱>/spritesheet.webp` | 下載或匯入的外觀 |

視窗位置、縮放比例存在 `~/.pawpeek/data/`。

外觀也會從 `~/.codex/pets/`、`~/.petdex/pets/` 讀取（與 Petdex 相容）。spritesheet 格式為 8 欄 × 9 或 11 列，png 或 webp。

## 架構

```
Claude Code hooks ──TCP 127.0.0.1:47390──▶ 狀態機（多對話合併）──▶ WebView（CSS 逐格動畫）
```

| 層 | 技術 |
|----|------|
| 視窗 | wry + tao（Windows WebView2 / macOS WKWebView） |
| 狀態 | Rust 狀態機，依優先順序合併多個對話 |
| Hook | Claude Code plugin hooks，見 [`hooks/hooks.json`](hooks/hooks.json) |
| 精靈 | Petdex spritesheet |

## 移除

```
/plugin uninstall pawpeek@pawpeek
```

要一併清除設定，刪除「文件」下的 `.pawpeek/` 與 `~/.pawpeek/`。

## 致謝

- [Jedeiah/agent-critter](https://github.com/Jedeiah/agent-critter) — 本專案的原始專案
- [Petdex](https://github.com/crafter-station/petdex) — 精靈格式與社群精靈
- [wry](https://github.com/tauri-apps/wry)、[tao](https://github.com/tauri-apps/tao) — Rust WebView 與視窗

## 開發方式

| 角色 | 負責 |
|------|------|
| BM.Liao | 想法、功能需求、使用測試與回饋 |
| AI（Anthropic Claude，Claude Code） | 程式碼實作、除錯、文件撰寫 |

git commit 中的 `Co-Authored-By: Claude` 標示了 AI 參與的部分。

## 授權

[MIT](LICENSE)。保留原專案版權聲明；PawPeek 的修改由 AI 依 BM.Liao 的需求完成。
