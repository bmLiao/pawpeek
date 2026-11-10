/// Petdex-verbatim HTML/CSS/JS.
/// `messages_js`：messages.json 合併後的 JSON（見 config::Messages::to_js）
pub fn build_page(messages_js: &str, saved_scale: f64) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body {{ margin: 0; padding: 0; background: transparent; overflow: hidden; width: 100%; height: 100%; font-family: -apple-system, system-ui, sans-serif; }}
  html, body, body * {{ -webkit-user-select: none; user-select: none; }}
  body input {{ -webkit-user-select: auto; user-select: auto; }}
  * {{ cursor: default !important; }}
  .stage {{ position: fixed; left: 0; top: 0; width: 100%; height: 100%; display: flex; align-items: center; justify-content: center; }}
  .pet {{
    aspect-ratio: 192 / 208;
    width: 7rem;
    image-rendering: pixelated;
    pointer-events: auto;
    cursor: grab;
  }}
  .pet.dragging {{ cursor: grabbing; }}
  ::-webkit-scrollbar {{ display: none; }}
{bubble_css}
</style>
</head>
<body>
<div class="stage"><div class="pet" id="pet" data-state="idle"><canvas id="pet-canvas" width="192" height="208" style="display:block;width:100%;height:100%;pointer-events:none;image-rendering:pixelated"></canvas></div></div>
<script>
var COLS = 8, ROWS = 9;
var STATES = {{
  idle:           {{ row: 0, frames: [{{c:0,d:280}},{{c:1,d:110}},{{c:2,d:110}},{{c:3,d:140}},{{c:4,d:140}},{{c:5,d:320}}], slow: 6 }},
  "running-right":{{ row: 1, count: 8, dur: 120, last: 220 }},
  "running-left": {{ row: 2, count: 8, dur: 120, last: 220 }},
  waving:         {{ row: 3, count: 4, dur: 140, last: 280 }},
  jumping:        {{ row: 4, count: 5, dur: 140, last: 280 }},
  failed:         {{ row: 5, count: 8, dur: 140, last: 240 }},
  waiting:        {{ row: 6, count: 6, dur: 150, last: 260 }},
  running:        {{ row: 7, count: 6, dur: 120, last: 220 }},
  review:         {{ row: 8, count: 6, dur: 150, last: 280 }},
}};
// 用實體按鍵（e.code）判斷，開中文輸入法時也有效
function keyName(e) {{
  var m = /^(?:Digit|Numpad)([0-9])$/.exec(e.code) || /^Key([A-Z])$/.exec(e.code);
  if (m) return m[1].toLowerCase();
  if (e.code === 'BracketLeft') return '[';
  if (e.code === 'BracketRight') return ']';
  return e.key;
}}
// 提示文字（來自 文件/.pawpeek/messages.json）
var MSG = {messages_js};
function M(group, key) {{ var g = MSG[group]; return (g && typeof g[key] === 'string') ? g[key] : ''; }}
function pickMsg(arr) {{
  if (typeof arr === 'string') return arr;
  if (!arr || !arr.length) return '';
  return arr[Math.floor(Math.random() * arr.length)];
}}
function fmt(s, vars) {{
  return String(s || '').replace(/\{{(\w+)\}}/g, function(m, k) {{ return (vars && k in vars) ? vars[k] : m; }});
}}
function buildFrames(s) {{
  if (s.frames) {{ var slow = s.slow || 1; return s.frames.map(function(f) {{ return {{ c: f.c, r: s.row, d: f.d * slow }}; }}); }}
  return Array.from({{length: s.count}}, function(_,i) {{ return {{ c: i, r: s.row, d: i === s.count - 1 ? s.last : s.dur }}; }});
}}
var pet = document.getElementById('pet');
// 用 canvas 只畫出單一格（精確裁切），避免縮放後的捨入誤差露出隔壁格的邊緣
var petCanvas = document.getElementById('pet-canvas');
var petCtx = petCanvas.getContext('2d');
var spriteImg = null, cellW = 192, cellH = 208;
var curFrame = {{ c: 0, r: 0 }};
function showFrame(c, r) {{
  curFrame = {{ c: c, r: r }};
  if (!spriteImg) return;
  petCtx.clearRect(0, 0, petCanvas.width, petCanvas.height);
  petCtx.drawImage(spriteImg, c * cellW, r * cellH, cellW, cellH, 0, 0, petCanvas.width, petCanvas.height);
}}
// 依圖片高度判斷列數：舊版 9 列（1536×1872）、新版 11 列（1536×2288，多 2 列看向方向）
// 格子固定 192×208，用寬度推算格高，與圖片實際解析度無關
window.loadSprite = function(url) {{
  var img = new Image();
  img.onload = function() {{
    var cellH = img.naturalWidth / COLS * 208 / 192;
    var rows = img.naturalHeight / cellH;
    var n = Math.round(rows);
    if (n < 9 || Math.abs(rows - n) > 0.01) {{
      console.warn('[pawpeek] 精靈圖尺寸不符 ' + img.naturalWidth + 'x' + img.naturalHeight + '，以 9 列處理');
      n = 9;
    }}
    ROWS = n;
    cellW = img.naturalWidth / COLS;
    cellH = img.naturalHeight / ROWS;
    spriteImg = img;
    showFrame(curFrame.c, curFrame.r);
  }};
  img.onerror = function() {{ console.warn('[pawpeek] 精靈圖載入失敗 ' + url); }};
  img.src = url;
}};
loadSprite('/sprite');
var stageEl = pet.parentElement;
if (stageEl) {{
  stageEl.style.position = 'fixed';
  stageEl.style.width = '100%';
  stageEl.style.height = '100%';
  stageEl.style.display = 'flex';
  stageEl.style.alignItems = 'center';
}}
var currentState = null, stateTimer = null;
// 防閃爍：每個動作至少播 MIN_HOLD_MS；期間的切換請求只保留最後一個，時間到再換
var MIN_HOLD_MS = 800, lastSwitchAt = 0, pendingState = null, pendingTimer = null;
// 目前動畫播完這一輪（回到第一格）時呼叫一次（拖曳放開後等動作播完才回待機）
var cycleCb = null;
function afterCycle(cb) {{ cycleCb = cb; }}
function play(state) {{
  if (pendingTimer) {{ clearTimeout(pendingTimer); pendingTimer = null; pendingState = null; }}
  if (state === currentState) return;
  var wait = lastSwitchAt + MIN_HOLD_MS - Date.now();
  if (currentState !== null && wait > 0) {{
    pendingState = state;
    pendingTimer = setTimeout(function() {{ var s = pendingState; pendingTimer = null; pendingState = null; play(s); }}, wait);
    return;
  }}
  lastSwitchAt = Date.now();
  currentState = state;
  pet.dataset.state = state;
  if (stateTimer) {{ clearTimeout(stateTimer); stateTimer = null; }}
  var def = STATES[state] || STATES.idle;
  var frames = buildFrames(def);
  var i = 0;
  showFrame(frames[0].c, frames[0].r);
  if (frames.length === 1) {{ if (cycleCb) {{ var f1 = cycleCb; cycleCb = null; f1(); }} return; }}
  (function tick() {{
    stateTimer = setTimeout(function() {{
      i = (i + 1) % frames.length;
      showFrame(frames[i].c, frames[i].r);
      if (i === 0 && cycleCb) {{
        var f = cycleCb; cycleCb = null; f();
        if (currentState !== state) return; // 回呼已換成別的動畫
      }}
      tick();
    }}, frames[i].d);
  }})();
}}
play('idle');

// --- Bridge ---
function stateLabel(s) {{
  var key = (s && s.indexOf('running') === 0) ? 'running' : s;
  return M('state_label', key) || s;
}}
window.__realState = 'idle';
window.__stateLabel = stateLabel('idle');
window.__sessions = 0;
window.__savedScale = {saved_scale};

// --- Bubble（泡泡與寵物同一個視窗，見 bubble.rs）---
window.__baseSize = {{ w: 140, h: 180 }};
{bubble_js}

// 縮放（[ / ] 與啟動時還原）
window.applyScale = function(s) {{
  window.__petScale = s;
  // 缩放整个页面（zoom 在 WKWebView 和 WebView2 均支持）
  document.body.style.zoom = s;
  var pw = Math.ceil(112 * s);
  var ph = Math.ceil(pw / 192 * 208);
  window.__baseSize = {{ w: pw + 40, h: ph + 60 }};
  if (window.resetBubbleReserve) window.resetBubbleReserve();
  // 寵物上下各留 30（螢幕像素）；body 有 zoom，所以除以 s
  if (stageEl) stageEl.style.padding = (30 / s) + 'px 0';
  window.syncWindowSize();
  window.ipc.postMessage(JSON.stringify({{type:'saveScale',scale:s}}));
}};

// Restore saved scale
if (window.__savedScale) window.applyScale(window.__savedScale);
else window.syncWindowSize();

{controller_js}

// 右鍵：擋掉瀏覽器預設選單
document.addEventListener('contextmenu', function(e) {{ e.preventDefault(); }});

// --- 鍵盤（點一下寵物取得焦點後可用）---
// [ / ]：縮小 / 放大，範圍 0.5～1.5
document.addEventListener('keydown', function(e) {{
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  var k = keyName(e);
  var d = 0;
  if (k === ']') d = 0.1;
  else if (k === '[') d = -0.1;
  if (!d) return;
  e.preventDefault();
  var s = Math.round(((window.__petScale || 1.0) + d) * 10) / 10;
  s = Math.min(1.5, Math.max(0.5, s));
  if (s !== window.__petScale) window.applyScale(s);
  var t = fmt(M('zoom','changed'), {{scale: s.toFixed(1)}});
  if (t) speech.special(t, 1500); // 縮放提示屬於特殊狀態
}});

</script>
</body></html>"#, messages_js=messages_js, saved_scale=saved_scale,
        bubble_css=crate::bubble::CSS, bubble_js=crate::bubble::JS, controller_js=CONTROLLER_JS)
}

/// 狀態與說話的控制（拖曳、單擊、雙擊、自言自語、更新提醒）。規則見 docs/BEHAVIOR.md，改動時一併更新。
/// 需要頁面先定義 `MSG`、`pickMsg`、`fmt`、`M`、`play`、`afterCycle`、`stateLabel`、`pet`，以及 bubble.rs 的 `speech`。
const CONTROLLER_JS: &str = r#"
// 各狀態用的動畫；dragging 是拖曳中（只能從待機進入），依滑鼠左右方向水平翻轉
var ANIM = { idle: 'idle', running: 'running', waiting: 'waiting', review: 'review', failed: 'failed', dragging: 'jumping' };
// 狀態 → messages.json 的 state_bubble 欄位（持久台詞）
var STATE_BUBBLE = { running: 'running', waiting: 'need_confirm', review: 'tool_error', failed: 'error_final' };
var IDLE_ACTIONS = ['jumping', 'waving', 'review'];
var CLICK_ACTIONS = ['waving', 'jumping', 'waiting', 'review'];
var CHATTER_MS = 4000;

var hookState = 'idle';      // Claude Code 的狀態
var mode = null;             // 目前顯示的狀態（hookState 或 dragging）
var modeSince = Date.now();  // 進入目前狀態的時間（自言自語從這裡起算）
var lastChatter = 0;
var pendingUpdate = null;    // 等第一次待機才顯示的更新提醒
var actionTimer = null;

// 拖曳方向：0 / 1 = 原圖（往右），-1 = 水平翻轉（往左）
var dragDir = 0, dragAccX = 0, DRAG_DIR_PX = 6;

function chatterPool(s) { var c = MSG.chatter || {}; return c[s] || []; }
function modeAnim() { return ANIM[mode] || 'idle'; }
function setFlip(on) { petCanvas.style.transform = on ? 'scaleX(-1)' : ''; }
// 拖曳中依滑鼠水平移動翻轉：同方向累積超過 DRAG_DIR_PX 才換（避免手抖左右閃），停下來保持最後方向
function trackDragDir(dx) {
  if (mode !== 'dragging' || !dx) return;
  dragAccX = (dx > 0) === (dragAccX > 0) ? dragAccX + dx : dx;
  if (Math.abs(dragAccX) < DRAG_DIR_PX) return;
  var d = dragAccX > 0 ? 1 : -1;
  if (d === dragDir) return;
  dragDir = d;
  setFlip(d < 0);
}

// 短暫動作（自言自語、單擊），結束後回到目前狀態的動畫
function playAction(a, ms) {
  play(a);
  clearTimeout(actionTimer);
  actionTimer = setTimeout(function() { actionTimer = null; play(modeAnim()); }, ms);
}

// 進入狀態：自言自語重新計時、換動畫、換台詞
//   工作中等狀態與拖曳：持久台詞，一直顯示到狀態改變
//   待機：沒有持久台詞，進入時說一句暫時台詞（entry）
function enterMode(m, entry) {
  mode = m; modeSince = Date.now(); lastChatter = modeSince;
  dragDir = 0; dragAccX = 0; setFlip(false);
  clearTimeout(actionTimer); actionTimer = null;
  play(modeAnim());
  var base = m === 'dragging' ? pickMsg(chatterPool('dragging'))
           : m === 'idle' ? '' : M('state_bubble', STATE_BUBBLE[m]);
  speech.enter(base, entry);
  if (m === 'idle' && pendingUpdate) {
    var t = pendingUpdate; pendingUpdate = null;
    speech.special(t, 0, true); // 顯示到狀態改變；期間待機的自言自語暫停
  }
}

// 主程式通知 Claude Code 狀態改變（prev 為 null 表示剛啟動）
window.onHookState = function(s, prev) {
  hookState = s;
  window.__realState = s;
  window.__stateLabel = stateLabel(s);
  // 轉回待機時仍按著寵物：直接接上拖曳
  if (s === 'idle' && dragHeld) return enterMode('dragging');
  var entry = null;
  if (s === 'idle' && prev) entry = pickMsg(prev === 'running' ? MSG.done : chatterPool('idle'));
  enterMode(s, entry);
};

// 有新版：已在待機就顯示，否則等第一次待機
window.queueUpdate = function(text) {
  if (mode === 'idle') speech.special(text, 0, true); else pendingUpdate = text;
};

// 自言自語：停留在同一個狀態才會說（拖曳中、特殊狀態期間不說）
setInterval(function() {
  if (mode === null || mode === 'dragging' || speech.busy()) return;
  var now = Date.now(), stay = now - modeSince;
  if (stay < 20000 || stay > 7200000 || now - lastChatter < 20000) return;
  var prob = stay < 300000 ? 0.4 : stay < 1800000 ? 0.15 : 0.05;
  if (Math.random() >= prob) return;
  if (!speech.say(pickMsg(chatterPool(mode)), CHATTER_MS)) return;
  lastChatter = now;
  if (mode === 'idle') playAction(IDLE_ACTIONS[Math.floor(Math.random() * IDLE_ACTIONS.length)], 5000);
}, 10000);

// --- 拖曳：按住超過 PRESS_MS 或開始移動就進入拖曳；比這短就放開算單擊 ---
var PRESS_MS = 150;
var mouseDown = false, dragHeld = false, wasDrag = false, skipClick = false, pressTimer = null;
var startX = 0, startY = 0, lastMove = 0;
function startDrag() {
  clearTimeout(pressTimer); pressTimer = null;
  if (!mouseDown || dragHeld) return;
  dragHeld = true;
  pet.classList.add('dragging');
  // 非待機：保留目前狀態的動畫與台詞，轉回待機時（仍按著）再接上拖曳
  if (hookState === 'idle') enterMode('dragging');
}
function endDrag() {
  dragHeld = false;
  pet.classList.remove('dragging');
  if (mode !== 'dragging') return;
  // 拖曳動作完整播完這一輪才回待機（拖曳屬於待機底下的動作，回待機不另外說話）
  afterCycle(function() { if (mode === 'dragging' && !dragHeld) enterMode('idle', null); });
}
document.body.addEventListener('mousedown', function(e) {
  if (e.button !== 0) return;
  // 只有按在寵物身上才拖曳（視窗含預留的泡泡空間，空白處不算）
  if (!(e.target === pet || pet.contains(e.target))) return;
  if (navigator.platform.indexOf('Mac') !== -1) return;
  mouseDown = true; wasDrag = false; skipClick = false;
  startX = e.screenX; startY = e.screenY; lastMove = 0;
  pressTimer = setTimeout(startDrag, PRESS_MS);
});
window.addEventListener('mousemove', function(e) {
  if (!mouseDown) return;
  var now = Date.now();
  if (now - lastMove < 16) return; // throttle to ~60fps
  lastMove = now;
  var dx = e.screenX - startX, dy = e.screenY - startY;
  if (Math.abs(dx) > 2 || Math.abs(dy) > 2) { wasDrag = true; startDrag(); }
  trackDragDir(dx);
  window.ipc.postMessage(JSON.stringify({type:'move',dx:dx,dy:dy}));
  startX = e.screenX; startY = e.screenY;
});
window.addEventListener('mouseup', function() {
  if (!mouseDown) return;
  mouseDown = false;
  clearTimeout(pressTimer); pressTimer = null;
  if (wasDrag) window.ipc.postMessage(JSON.stringify({type:'savePos'}));
  if (dragHeld) { skipClick = true; endDrag(); }
});

// --- 單擊：依目前狀態說一句自言自語（待機時加一個小動作） ---
// 等 CLICK_WAIT_MS 確定不是雙擊才算單擊（雙擊時瀏覽器會先送兩次 click）
var CLICK_WAIT_MS = 250;
var clickBusy = false, clickTimer = null;
pet.addEventListener('click', function() {
  if (skipClick || wasDrag) { skipClick = false; return; }
  if (clickTimer) return; // 第二下：交給 dblclick
  clickTimer = setTimeout(function() { clickTimer = null; singleClick(); }, CLICK_WAIT_MS);
});
function singleClick() {
  if (clickBusy) return;
  var m = (mode === 'dragging' || !mode) ? 'idle' : mode;
  if (!speech.say(pickMsg(chatterPool(m)), CHATTER_MS)) return; // 特殊狀態期間不說
  if (m === 'idle') {
    clickBusy = true;
    playAction(CLICK_ACTIONS[Math.floor(Math.random() * CLICK_ACTIONS.length)], 2000);
    setTimeout(function() { clickBusy = false; }, 2000);
  }
}

// --- 雙擊：回報對話數與狀態（特殊狀態期間不說） ---
pet.addEventListener('dblclick', function() {
  clearTimeout(clickTimer); clickTimer = null; // 取消等待中的單擊
  if (wasDrag) return;
  var state = window.__realState || 'idle';
  var count = window.__sessions || 0;
  var key;
  if (state === 'idle') key = count === 0 ? 'idle_none' : 'idle';
  else if (state.indexOf('running') !== -1) key = 'running';
  else if (state === 'waiting' || state === 'review' || state === 'failed') key = state;
  else key = 'other';
  var msg = fmt(M('dblclick', key), {count: count, state: window.__stateLabel || state});
  speech.say(msg, 3000);
});
"#;

pub fn build_empty_page(message: &str) -> String {
    let msg_json = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    format!(r#"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;padding:0;background:transparent;width:100%;height:100%;font-family:-apple-system,sans-serif;-webkit-app-region:drag;display:flex;align-items:center;justify-content:center}}
p{{color:rgba(255,255,255,0.6);font-size:9px;text-align:center;white-space:pre-line;-webkit-app-region:no-drag}}
</style></head><body><p>{msg}</p></body></html>"#, msg=msg_json)
}
