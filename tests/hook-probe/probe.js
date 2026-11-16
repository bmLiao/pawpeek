#!/usr/bin/env node
// Hook 探測器：把每次被呼叫時的參數、stdin、環境變數記成一行 JSON，用來確認各家 agent（Claude Code、
// VS Code Copilot、Copilot CLI）實際怎麼執行 hook、送什麼資料。輸出 `{}` 到 stdout（不做任何決定）。
// log 位置：環境變數 HOOK_PROBE_LOG，否則 <暫存>/hook-probe/log.jsonl
const fs = require('fs');
const os = require('os');
const path = require('path');

const logFile = process.env.HOOK_PROBE_LOG || path.join(os.tmpdir(), 'hook-probe', 'log.jsonl');
const ENV_KEYS = ['CLAUDE_PLUGIN_ROOT', 'PLUGIN_ROOT', 'CLAUDE_PROJECT_DIR', 'COPILOT_HOME', 'TERM_PROGRAM',
  'VSCODE_PID', 'SHELL', 'ComSpec', 'PSModulePath', 'MSYSTEM', 'CLAUDECODE', 'COPILOT_CLI', 'COPILOT_AGENT'];

function write(stdin) {
  let parsed = null;
  try { parsed = stdin ? JSON.parse(stdin) : null; } catch (_) {}
  const env = {};
  for (const k of ENV_KEYS) if (process.env[k] !== undefined) env[k] = k === 'PSModulePath' ? '(set)' : process.env[k];
  // 只記名稱：看看 agent 還塞了哪些環境變數
  const extra = Object.keys(process.env).filter(k => /claude|copilot|vscode|hook|plugin/i.test(k) && !ENV_KEYS.includes(k));
  const rec = { t: new Date().toISOString(), tag: process.argv[2] || '(no-tag)', argv: process.argv.slice(2),
    cwd: process.cwd(), ppid: process.ppid, env, extraEnv: extra, stdin: parsed || stdin || null };
  fs.mkdirSync(path.dirname(logFile), { recursive: true });
  fs.appendFileSync(logFile, JSON.stringify(rec) + '\n');
  process.stdout.write('{}');
  process.exit(0);
}

// stdin 可能沒有關閉：最多等 1.5 秒
let buf = '';
const timer = setTimeout(() => write(buf), 1500);
process.stdin.setEncoding('utf8');
process.stdin.on('data', d => { buf += d; });
process.stdin.on('end', () => { clearTimeout(timer); write(buf); });
process.stdin.on('error', () => { clearTimeout(timer); write(buf); });
