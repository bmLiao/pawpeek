// 列出 PAWPEEK_HOOK_LOG 的內容：時間、事件、對應到的桌寵事件、session、是哪個執行檔
// 用法：node pawpeek-log.js <pawpeek-*.jsonl> [--raw 事件名]
const fs = require('fs');
const [file, flag, rawName] = process.argv.slice(2);
const recs = fs.readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).map(l => JSON.parse(l));
const name = r => (r.input && (r.input.hook_event_name || r.input.hookName)) || '(empty)';
if (flag === '--raw') {
  for (const r of recs.filter(r => name(r) === rawName)) console.log(JSON.stringify(r.input, null, 1).slice(0, 1500));
  process.exit(0);
}
for (const r of recs) {
  // 啟動常駐程式的紀錄（daemon::start_detached_daemon）：breakaway / wmi / detached
  if (r.spawn) { console.log(new Date(r.t).toISOString().slice(11, 19), '  ↳ 啟動桌寵'.padEnd(22), '->', r.spawn); continue; }
  console.log(new Date(r.t).toISOString().slice(11, 19), name(r).padEnd(22), '->', String(r.mapped).padEnd(14),
    String(r.session || '').slice(0, 8), r.exe ? require('path').basename(require('path').dirname(require('path').dirname(r.exe))) : '');
}
