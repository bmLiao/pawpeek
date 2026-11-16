// 產生 hooks/hooks.json：每個事件掛多種寫法，log 的 tag 標出是哪一種被執行
//   v1-cmd      command 字串內帶參數（node + 腳本路徑）
//   v2-args     command + args 陣列（PawPeek 目前的寫法）
//   v3-sh       無副檔名 sh 啟動器（PawPeek 的 bin/pawpeek 形式）
//   v4-ps       command 與 powershell 兩個欄位都給，看 Windows 用哪個（v4-fallback / v4-ps）
//   v5-cmdfile  Windows .cmd 檔
//   v6-async    v1 加上 "async": true
//   v7-matcher  SessionStart 加上 PawPeek 目前的 matcher
// 用法：node gen-hooks.js
const fs = require('fs');
const path = require('path');

const EVENTS = ['SessionStart', 'UserPromptSubmit', 'PreToolUse', 'PostToolUse', 'PostToolUseFailure', 'PreCompact',
  'Stop', 'StopFailure', 'Notification', 'PermissionRequest', 'ErrorOccurred', 'SessionEnd', 'SubagentStart', 'SubagentStop'];
const R = '${CLAUDE_PLUGIN_ROOT}';
const node = (tag, ev) => `node "${R}/probe.js" ${tag} ${ev}`;

const variants = ev => [
  { type: 'command', command: node('v1-cmd', ev), timeout: 10 },
  { type: 'command', command: 'node', args: [`${R}/probe.js`, 'v2-args', ev], timeout: 10 },
  { type: 'command', command: `${R}/bin/probe v3-sh ${ev}`, timeout: 10 },
  { type: 'command', command: node('v4-fallback', ev), powershell: node('v4-ps', ev), timeout: 10 },
  { type: 'command', command: `"${R}/bin/probe.cmd" v5-cmdfile ${ev}`, timeout: 10 },
  { type: 'command', command: node('v6-async', ev), async: true, timeout: 10 },
];

const hooks = {};
for (const ev of EVENTS) {
  hooks[ev] = [{ hooks: variants(ev) }];
  if (ev === 'SessionStart') {
    hooks[ev].push({ matcher: 'startup|resume|clear|compact', hooks: [{ type: 'command', command: node('v7-matcher', ev), timeout: 10 }] });
  }
}
const out = path.join(__dirname, 'hooks', 'hooks.json');
fs.mkdirSync(path.dirname(out), { recursive: true });
fs.writeFileSync(out, JSON.stringify({ description: 'hook-probe', hooks }, null, 2) + '\n');
console.log('wrote', out);
