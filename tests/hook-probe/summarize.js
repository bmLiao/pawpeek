// 整理探測 log：每個事件 × 寫法的執行次數，以及每個事件收到的 stdin 欄位
// 用法：node summarize.js <log.jsonl>
const fs = require('fs');
const file = process.argv[2];
const recs = fs.readFileSync(file, 'utf8').trim().split('\n').filter(Boolean).map(l => JSON.parse(l));
const TAGS = ['v1-cmd', 'v2-args', 'v3-sh', 'v4-fallback', 'v4-ps', 'v5-cmdfile', 'v6-async', 'v7-matcher'];
const table = {};
const fields = {};
for (const r of recs) {
  const ev = r.argv[1] || '(none)';
  (table[ev] = table[ev] || {})[r.tag] = (table[ev][r.tag] || 0) + 1;
  if (r.stdin && typeof r.stdin === 'object') {
    const f = fields[ev] = fields[ev] || {};
    for (const [k, v] of Object.entries(r.stdin)) {
      const s = typeof v === 'object' ? JSON.stringify(v) : String(v);
      f[k] = s.length > 70 ? s.slice(0, 70) + '…' : s;
    }
  }
}
console.log('event'.padEnd(20) + TAGS.map(t => t.padStart(12)).join(''));
for (const [ev, row] of Object.entries(table)) console.log(ev.padEnd(20) + TAGS.map(t => String(row[t] || '-').padStart(12)).join(''));
console.log('\n# stdin fields per event');
for (const [ev, f] of Object.entries(fields)) { console.log(`\n[${ev}]`); for (const [k, v] of Object.entries(f)) console.log(`  ${k} = ${v}`); }
const r0 = recs[0];
if (r0) console.log('\n# sample env', JSON.stringify(r0.env), '\n# extraEnv', JSON.stringify(r0.extraEnv), '\n# argv', JSON.stringify(r0.argv), '\n# cwd', r0.cwd);
