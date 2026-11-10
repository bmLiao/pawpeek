---
description: 切換桌寵外觀（Petdex 名稱、圖片路徑或 default；不帶參數列出可用外觀）
argument-hint: "[Petdex 名稱 | 圖片路徑 | default]"
---

使用者給的參數：`$ARGUMENTS`

用 Bash 工具執行下面這行，把上面的參數原封不動放進雙引號裡（參數是空的就留空字串 `""`；不要自己處理圖片或下載）：

```
"${CLAUDE_PLUGIN_ROOT}/bin/pawpeek" --set-skin "<參數>"
```

下載、格式檢查、寫入設定與桌寵換圖都由程式自己處理。
執行完只用一兩句繁體中文轉述指令輸出的結果，不要做其他事。
