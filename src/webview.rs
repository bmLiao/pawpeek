/// Petdex-verbatim HTML/CSS/JS.
/// `messages_js`：messages.json 合併後的 JSON（見 config::Messages::to_js）
pub fn build_page(messages_js: &str, saved_scale: f64) -> String {
    format!(r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body {{ margin: 0; padding: 0; background: transparent; overflow: hidden; width: 100%; height: 100%; font-family: -apple-system, system-ui, sans-serif; }}
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
var currentState = 'idle', stateTimer = null;
function play(state) {{
  if (state === currentState) return;
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
window.setState = function(state, durationMs) {{
  play(state);
  if (window.__stateTimer) clearTimeout(window.__stateTimer);
  if (durationMs) {{
    window.__stateTimer = setTimeout(function() {{
      play(window.__realState || 'idle');
      window.__stateTimer = null;
    }}, durationMs);
  }}
}};
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

// Global scale function (used by menu and restore)
window.applyScale = function(s) {{
  window.__petScale = s;
  // 缩放整个页面（zoom 在 WKWebView 和 WebView2 均支持）
  document.body.style.zoom = s;
  var sizeLabel = document.getElementById('size-label');
  if (sizeLabel) sizeLabel.textContent = M('menu','size')+' x'+s.toFixed(1);
  var pw = Math.ceil(112 * s);
  var ph = Math.ceil(pw / 192 * 208);
  window.__baseSize = {{ w: pw + 40, h: ph + 60 }};
  // 寵物上下各留 30（螢幕像素）；body 有 zoom，所以除以 s
  if (stageEl) stageEl.style.padding = (30 / s) + 'px 0';
  window.syncWindowSize();
  window.ipc.postMessage(JSON.stringify({{type:'saveScale',scale:s}}));
}};

// Restore saved scale
if (window.__savedScale) window.applyScale(window.__savedScale);

// --- Drag: mousedown anywhere on window → move window ---
var dragging = false, wasDrag = false, startX = 0, startY = 0, lastMove = 0;
document.body.addEventListener('mousedown', function(e) {{
  if (e.button !== 0) return;
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
    var dragMsg = (!window.__realState || window.__realState === 'idle') ? M('drag','idle') : M('drag','busy');
    window.showTransientBubble(dragMsg, 2000);
    if (!window.__realState || window.__realState === 'idle') {{
      window.setState('waving', 2000);
    }}
  }}
}});

// --- Right-click menu (list) ---
// 右鍵選單暫時關閉：改成 true 就恢復
window.__menuEnabled = false;
pet.addEventListener('contextmenu', function(e) {{
  e.preventDefault();
  if (!window.__menuEnabled) return;
  window.ipc.postMessage(JSON.stringify({{act:'1'}}));
  // Activate the window so timers & blur work
  window.ipc.postMessage(JSON.stringify({{type:'focus'}}));
  var menu = document.getElementById('pet-menu');
  if (!menu) {{
    menu = document.createElement('div');
    menu.id = 'pet-menu';
    menu.style.cssText = 'position:fixed;background:rgba(20,20,22,0.96);border:1px solid rgba(255,255,255,0.08);border-radius:10px;padding:10px;z-index:999;left:4px;right:4px;top:4px;bottom:4px;overflow-y:auto;overflow-x:hidden;scrollbar-width:none;-ms-overflow-style:none;pointer-events:auto;display:none';
    document.body.appendChild(menu);
    // Dismiss on outside click or window blur
    document.addEventListener('click', function(ev) {{ if (menu && menu.style.display === 'block' && !menu.contains(ev.target) && ev.target !== pet) {{ menu.style.display = 'none'; document.body.style.pointerEvents = 'none'; }} }});
    window.addEventListener('blur', function() {{ if (menu) {{ menu.style.display = 'none'; document.body.style.pointerEvents = 'none'; }} }});
  }}
  menu.innerHTML = '';
  function hideMenu() {{ menu.style.display = 'none'; document.body.style.pointerEvents = 'none'; }}
  // 選單項目：整列可點、左對齊、hover 高亮
  function addItem(label, color, onClick) {{
    var it = document.createElement('div');
    it.textContent = label;
    it.style.cssText = 'padding:6px 10px;border-radius:6px;color:' + (color || '#ddd') + ';cursor:pointer;font-size:11px;text-align:left;white-space:nowrap;overflow:hidden;text-overflow:ellipsis';
    it.addEventListener('mouseenter', function() {{ it.style.background = 'rgba(255,255,255,0.08)'; }});
    it.addEventListener('mouseleave', function() {{ it.style.background = ''; }});
    it.addEventListener('click', function(ev) {{ ev.stopPropagation(); onClick(); }});
    menu.appendChild(it);
    return it;
  }}

  // Size control（程式保留，暫不顯示於 UI；改 true 即恢復）
  var SHOW_SIZE_CONTROL = false;
  window.__petScale = window.__petScale || 1.0;
  if (SHOW_SIZE_CONTROL) {{
  var sizeLabel = document.createElement('div');
  sizeLabel.id = 'size-label';
  sizeLabel.textContent = M('menu','size') + ' x' + window.__petScale.toFixed(1);
  sizeLabel.style.cssText = 'padding:3px 8px;color:rgba(255,255,255,0.4);font-size:9px;text-align:center';
  menu.appendChild(sizeLabel);
  var sizeRow = document.createElement('div');
  sizeRow.style.cssText = 'display:flex;gap:4px;justify-content:center;padding:2px 0';
  var minus = document.createElement('div');
  minus.textContent = '−'; minus.style.cssText = 'width:22px;text-align:center;color:#aaa;cursor:pointer;font-size:12px;border-radius:3px';
  minus.addEventListener('mouseenter', function(){{ minus.style.background='rgba(255,255,255,0.1)'; }});
  minus.addEventListener('mouseleave', function(){{ minus.style.background=''; }});
  var plus = document.createElement('div');
  plus.textContent = '+'; plus.style.cssText = 'width:22px;text-align:center;color:#aaa;cursor:pointer;font-size:12px;border-radius:3px';
  plus.addEventListener('mouseenter', function(){{ plus.style.background='rgba(255,255,255,0.1)'; }});
  plus.addEventListener('mouseleave', function(){{ plus.style.background=''; }});
  minus.addEventListener('click', function(e){{ e.stopPropagation(); window.applyScale(Math.max(0.5, window.__petScale - 0.1)); }});
  plus.addEventListener('click', function(e){{ e.stopPropagation(); window.applyScale(Math.min(1.5, window.__petScale + 0.1)); }});
  sizeRow.appendChild(minus); sizeRow.appendChild(plus);
  menu.appendChild(sizeRow);
  var hr2 = document.createElement('hr');
  hr2.style.cssText = 'margin:4px 0;border:none;border-top:1px solid rgba(255,255,255,0.06)';
  menu.appendChild(hr2);
  }}

  // 選單項目
  addItem(M('menu','quit'), '#f88', function() {{ window.ipc.postMessage('quit'); hideMenu(); }});
  document.body.style.pointerEvents = 'auto';
  menu.style.display = 'block';
  menu.style.top = '4px';
  e.stopPropagation();
  // Auto-close after 3s (reset on hover)
  clearTimeout(window.__menuTimer);
  window.__menuTimer = setTimeout(function() {{ menu.style.display = 'none'; document.body.style.pointerEvents = 'none'; }}, 3000);
  menu.onmouseenter = function() {{ clearTimeout(window.__menuTimer); }};
  menu.onmouseleave = function() {{ window.__menuTimer = setTimeout(function() {{ menu.style.display = 'none'; document.body.style.pointerEvents = 'none'; }}, 1500); }};
}});

// --- 鍵盤（點一下寵物取得焦點後可用）---
// +/-：縮放，範圍與選單相同 0.5～1.5；c：回到作用中視窗旁（找不到就主螢幕中央）
document.addEventListener('keydown', function(e) {{
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  if (e.key === 'c' || e.key === 'C') {{
    e.preventDefault();
    window.ipc.postMessage(JSON.stringify({{type:'center'}}));
    var ct = M('position','home');
    if (ct) window.showTransientBubble(ct, 1500);
    return;
  }}
  var d = 0;
  if (e.key === '+' || e.key === '=' || e.code === 'NumpadAdd') d = 0.1;
  else if (e.key === '-' || e.key === '_' || e.code === 'NumpadSubtract') d = -0.1;
  if (!d) return;
  e.preventDefault();
  var s = Math.round(((window.__petScale || 1.0) + d) * 10) / 10;
  s = Math.min(1.5, Math.max(0.5, s));
  if (s !== window.__petScale) window.applyScale(s);
  var t = fmt(M('zoom','changed'), {{scale: s.toFixed(1)}});
  if (t) window.showTransientBubble(t, 1500);
}});

// 滑鼠離開桌寵：把鍵盤焦點還給原本的程式，不打斷使用者工作
document.addEventListener('mouseout', function(e) {{
  if (e.relatedTarget || !window.__hasKeyFocus) return;
  window.__hasKeyFocus = false;
  window.ipc.postMessage(JSON.stringify({{type:'release'}}));
}});

// --- Single-click: position-based interaction ---
var clickActions = ['waving','jumping','waiting','review'];
pet.addEventListener('click', function(e) {{
  if (wasDrag) return; // 拖曳不取得焦點
  // 取得鍵盤焦點（視窗設了 NOACTIVATE，點擊不會自動取得）；滑鼠離開時還回去
  window.ipc.postMessage(JSON.stringify({{act:'1'}}));
  window.__hasKeyFocus = true;
  if (window.__realState && window.__realState !== 'idle') return;
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
  window.setState(a, 2000);
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
