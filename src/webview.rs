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
  if (frames.length === 1) return;
  (function tick() {{
    stateTimer = setTimeout(function() {{
      i = (i + 1) % frames.length;
      showFrame(frames[i].c, frames[i].r);
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
window.__stateTimer = null;
function backState() {{ return window.__realState || 'idle'; }}
function isIdle() {{ return backState() === 'idle'; }}
// 主程式（hook）呼叫的入口
window.setState = function(state, durationMs) {{
  localSetState(state, durationMs);
}};
function localSetState(state, durationMs) {{
  play(state);
  if (window.__stateTimer) clearTimeout(window.__stateTimer);
  if (durationMs) {{
    window.__stateTimer = setTimeout(function() {{
      window.__stateTimer = null;
      play(backState());
    }}, durationMs);
  }}
}}
window.setHookState = function(state) {{
  window.__realState = state;
  window.__stateLabel = stateLabel(state);
}};
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

// --- Drag: mousedown anywhere on window → move window ---
var dragging = false, wasDrag = false, startX = 0, startY = 0, lastMove = 0;
document.body.addEventListener('mousedown', function(e) {{
  if (e.button !== 0) return;
  // 只有按在寵物身上才拖曳（視窗含預留的泡泡空間，空白處不算）
  if (!(e.target === pet || pet.contains(e.target))) return;
  if (navigator.platform.indexOf('Mac') !== -1 && (e.target === pet || pet.contains(e.target))) return;
  dragging = true; wasDrag = false;
  startX = e.screenX; startY = e.screenY; lastMove = 0;
}});
window.addEventListener('mousemove', function(e) {{
  if (!dragging) return;
  var now = Date.now();
  if (now - lastMove < 16) return; // throttle to ~60fps
  lastMove = now;
  var dx = e.screenX - startX, dy = e.screenY - startY;
  if (Math.abs(dx) > 2 || Math.abs(dy) > 2) wasDrag = true;
  window.ipc.postMessage(JSON.stringify({{type:'move',dx:dx,dy:dy}}));
  startX = e.screenX; startY = e.screenY;
}});
window.addEventListener('mouseup', function() {{
  if (!dragging) return;
  dragging = false;
  window.ipc.postMessage(JSON.stringify({{type:'savePos'}}));
  if (wasDrag) {{
    var dragMsg = isIdle() ? M('drag','idle') : M('drag','busy');
    window.showTransientBubble(dragMsg, 2000);
    if (isIdle()) {{
      localSetState('waving', 2000);
    }}
  }}
}});

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
  if (t) window.showTransientBubble(t, 1500);
}});

// --- Single-click: position-based interaction ---
var clickActions = ['waving','jumping','waiting','review'];
pet.addEventListener('click', function(e) {{
  if (wasDrag) return;
  if (!isIdle()) return;
  if (window.__clickBusy) return;
  window.__clickBusy = true;
  var rect = pet.getBoundingClientRect();
  var y = (e.clientY - rect.top) / rect.height;
  var t, a;
  if (y < 0.3) {{
    t = pickMsg(MSG.click_head);
    a = 'waving';
  }} else {{
    t = pickMsg(MSG.click_body);
    a = clickActions[Math.floor(Math.random() * clickActions.length)];
  }}
  if (t) window.showTransientBubble(t, 3000);
  localSetState(a, 2000);
  setTimeout(function() {{ window.__clickBusy = false; }}, 2000);
}});

// --- Double-click shows status with session count ---
pet.addEventListener('dblclick', function(e) {{
  var state = window.__realState || 'idle';
  var count = window.__sessions || 0;
  var key;
  if (state === 'idle') key = count === 0 ? 'idle_none' : 'idle';
  else if (state.indexOf('running') !== -1) key = 'running';
  else if (state === 'waiting' || state === 'review' || state === 'failed') key = state;
  else key = 'other';
  var msg = fmt(M('dblclick', key), {{count: count, state: window.__stateLabel || state}});
  if (msg) window.showTransientBubble(msg, 3000);
}});

</script>
</body></html>"#, messages_js=messages_js, saved_scale=saved_scale,
        bubble_css=crate::bubble::CSS, bubble_js=crate::bubble::JS)
}

pub fn build_empty_page(message: &str) -> String {
    let msg_json = serde_json::to_string(message).unwrap_or_else(|_| "\"\"".into());
    format!(r#"<!DOCTYPE html><html><head><meta charset="utf-8"><style>
html,body{{margin:0;padding:0;background:transparent;width:100%;height:100%;font-family:-apple-system,sans-serif;-webkit-app-region:drag;display:flex;align-items:center;justify-content:center}}
p{{color:rgba(255,255,255,0.6);font-size:9px;text-align:center;white-space:pre-line;-webkit-app-region:no-drag}}
</style></head><body><p>{msg}</p></body></html>"#, msg=msg_json)
}
