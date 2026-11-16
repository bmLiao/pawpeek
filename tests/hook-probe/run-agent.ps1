# 用真的 agent（Copilot CLI 或 Claude Code）實測 PawPeek 的 hook，不動到已安裝的插件
#   1. 把 repo 的 .claude-plugin / hooks / commands / bin\pawpeek 與 target\<profile>\pawpeek.exe 組成測試插件
#   2. 關掉所有桌寵（其他 Claude Code 對話的 hook 會一直把已安裝版叫回來）
#   3. 以 -p 非互動模式跑提示詞，--plugin-dir 載入測試插件；PAWPEEK_HOOK_LOG 記錄每次 hook 收到的資料與對應事件
#   4. 印出 log 摘要
# 用法：
#   powershell -File tests\hook-probe\run-agent.ps1 -Out <資料夾> [-Agent copilot|claude] [-Prompt "..."] [-AllowAll] [-Profile debug|release] [-Probe]
#   -AllowAll：自動允許所有工具（copilot --allow-all / claude --dangerously-skip-permissions）
#   -Probe   ：改用 tests\hook-probe 探測插件（記錄 hook 的執行方式），不用 PawPeek
param(
  [Parameter(Mandatory = $true)][string]$Out,
  [ValidateSet("copilot", "claude")][string]$Agent = "copilot",
  [string]$Prompt = "Do exactly these steps: 1) run the shell command: echo probe-ok  2) create a file named hello.txt containing the text hi  3) run the shell command: exit 3  . Then reply DONE.",
  [switch]$AllowAll,
  [string]$Profile = "debug",
  [switch]$Probe
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path "$PSScriptRoot\..\..").Path

# 只關測試插件的桌寵（$plugin\bin 底下的執行檔與它的 WebView2），不動使用者正在用的已安裝版
# 測試版（debug build）預設用 47391 埠、顯示 DEMO，可以和已安裝的正式版（47390）同時開
function Stop-Pets {
  if (-not $plugin) { return }
  $bin = Join-Path $plugin "bin"
  Get-CimInstance Win32_Process | Where-Object {
    ($_.Name -eq 'pawpeek.exe' -and $_.ExecutablePath -like "$bin\*") -or
    ($_.Name -eq 'msedgewebview2.exe' -and $_.CommandLine -like "*$bin\pawpeek.exe.WebView2*")
  } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
  Start-Sleep -Milliseconds 800
}
New-Item -ItemType Directory -Force $Out | Out-Null
# 短檔名（例如 BMFE22~1.LIA）會讓 Push-Location 失敗：轉成長檔名
Add-Type -Namespace PawPeekTest -Name Path -MemberDefinition '[DllImport("kernel32.dll", CharSet=CharSet.Unicode)] public static extern uint GetLongPathName(string s, System.Text.StringBuilder b, uint n);'
$sb = New-Object System.Text.StringBuilder 1024
if ([PawPeekTest.Path]::GetLongPathName($Out, $sb, 1024) -gt 0) { $Out = $sb.ToString() }
$ws = Join-Path $Out "workspace"
New-Item -ItemType Directory -Force $ws | Out-Null

if ($Probe) {
  $plugin = "$repo\tests\hook-probe"
} else {
  $plugin = Join-Path $Out "plugin"
  foreach ($d in ".claude-plugin", "hooks", "commands", "bin") { New-Item -ItemType Directory -Force (Join-Path $plugin $d) | Out-Null }
  Copy-Item "$repo\.claude-plugin\plugin.json" "$plugin\.claude-plugin\" -Force
  Copy-Item "$repo\hooks\hooks.json" "$plugin\hooks\" -Force
  Copy-Item "$repo\commands\*" "$plugin\commands\" -Force -Recurse
  Copy-Item "$repo\bin\pawpeek" "$plugin\bin\pawpeek" -Force
  Stop-Pets
  Copy-Item "$repo\target\$Profile\pawpeek.exe" "$plugin\bin\pawpeek.exe" -Force
  # 執行檔要和 WebView2Loader.dll 放在一起，否則啟動失敗（0xC0000135 找不到 DLL）
  Copy-Item "$repo\target\$Profile\WebView2Loader.dll" "$plugin\bin\" -Force
  # 新複製的執行檔第一次執行會被防毒完整掃描（debug 版 100MB 要好幾秒），超過 hook 逾時會漏掉開頭的事件：先跑一次暖身
  & "$plugin\bin\pawpeek.exe" --quit | Out-Null
}

$stamp = Get-Date -Format "HHmmss"
$env:PAWPEEK_HOOK_LOG = Join-Path $Out "pawpeek-$Agent-$stamp.jsonl"
$env:HOOK_PROBE_LOG = Join-Path $Out "probe-$Agent-$stamp.jsonl"

if ($Agent -eq "copilot") {
  $exe = (Get-Command copilot.cmd -ErrorAction SilentlyContinue).Source
  if (-not $exe) { $exe = "$env:USERPROFILE\scoop\apps\fnm\current\node-versions\v22.22.3\installation\copilot.cmd" }
  $env:PATH = (Split-Path $exe) + ";" + $env:PATH
  $cliArgs = @("-C", $ws, "--plugin-dir", $plugin, "-p", $Prompt)
  if ($AllowAll) { $cliArgs += "--allow-all" }
} else {
  $exe = (Get-Command claude -ErrorAction Stop).Source
  # 停用已安裝的 PawPeek，只留測試插件（否則已安裝版的 hook 可能先叫起正式版常駐程式）
  # JSON 字串傳給 claude.ps1 時引號會被吃掉：寫成檔案再傳路徑
  $settings = Join-Path $Out "claude-settings.json"
  [IO.File]::WriteAllText($settings, '{"enabledPlugins":{"pawpeek@pawpeek":false}}')
  $cliArgs = @("-p", $Prompt, "--plugin-dir", $plugin, "--settings", $settings)
  if ($AllowAll) { $cliArgs += "--dangerously-skip-permissions" }
  # 從 Claude Code 裡面再開 Claude Code：清掉父對話的環境變數
  Get-ChildItem env: | Where-Object { $_.Name -eq 'CLAUDECODE' -or $_.Name -like 'CLAUDE_CODE_*' } | ForEach-Object { Remove-Item "env:$($_.Name)" }
}

"agent  : $Agent ($exe)"
"plugin : $plugin"
"log    : $(if ($Probe) { $env:HOOK_PROBE_LOG } else { $env:PAWPEEK_HOOK_LOG })"
"---- $Agent output (tail)"
$ErrorActionPreference = "Continue"  # agent 會寫 stderr，不能當成錯誤中斷
if (-not $Probe) { Stop-Pets }       # 啟動前再關一次：避免事件送到剛被其他對話叫回來的舊版
Push-Location -LiteralPath $ws
$res = & $exe @cliArgs 2>&1
$code = $LASTEXITCODE
Pop-Location
$res | ForEach-Object { "$_" } | Where-Object { $_ -notmatch 'RemoteException' } | Select-Object -Last 12
"exit=$code"
"---- running pets"
Get-CimInstance Win32_Process -Filter "Name='pawpeek.exe'" | ForEach-Object { "  $($_.ProcessId) $($_.ExecutablePath)" }
"---- hook log"
if ($Probe) {
  if (Test-Path $env:HOOK_PROBE_LOG) { node "$PSScriptRoot\summarize.js" $env:HOOK_PROBE_LOG } else { "  (no log)" }
} else {
  if (Test-Path $env:PAWPEEK_HOOK_LOG) { node "$PSScriptRoot\pawpeek-log.js" $env:PAWPEEK_HOOK_LOG } else { "  (no log)" }
}
