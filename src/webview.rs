/// Petdex-verbatim HTML/CSS/JS.
/// `messages_js`：messages.json 合併後的 JSON（見 config::Messages::to_js）
pub fn build_page(messages_js: &str, saved_scale: f64) -> String {
    // 測試版（debug build，`cargo build`）顯示不消失的 DEMO；正式版（--release）不顯示。見 docs/BEHAVIOR.md §9
    let demo_label = if cfg!(debug_assertions) {
        // 外框與泡泡相同（底色、圓角、陰影），位置由 CONTROLLER_JS 放到寵物方框內的底部
        r#"<div id="demo-label" style="position:fixed;transform:translateX(-50%);box-sizing:border-box;padding:3px 12px;border-radius:999px;background:rgba(22,24,38,0.92);color:#fff;border:1px solid rgba(255,255,255,0.10);font:700 12px/1.4 'Segoe UI','Microsoft JhengHei UI',system-ui,sans-serif;letter-spacing:1px;box-shadow:0 4px 14px rgba(0,0,0,0.32),0 1px 3px rgba(0,0,0,0.2);pointer-events:none;white-space:nowrap;z-index:5">DEMO</div>"#.to_string()
    } else { String::new() };
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
  /* F3 小寵物：放在 .pet 裡（跟著縮放），疊在寵物右下角上方；位置由 JS 每次打開時隨機（%，相對寵物方框） */
  .pet {{ position: relative; }}
  #minis {{ display: none; position: absolute; left: 0; top: 0; width: 100%; height: 100%; pointer-events: none; z-index: 1; }}
  body.minis #minis {{ display: block; }}
  #minis canvas {{ position: absolute; width: 28%; height: 28%; image-rendering: pixelated; pointer-events: none; }}
  #pet-bubble, .pp-panel {{ position: relative; z-index: 2; }}
  ::-webkit-scrollbar {{ display: none; }}
{bubble_css}
  /* 面板（F1 換外觀、F4 快捷鍵說明、Esc 離開）：放在泡泡的位置（寵物上方，靠近螢幕頂端時在下方），
     一次只開一個（body 的 data-panel），開著時泡泡先藏起來 */
  .pp-panel {{
    display: none; flex: 0 0 auto; box-sizing: border-box; width: 240px; margin-bottom: 9px; padding: 8px;
    border-radius: 12px; background: rgba(22, 24, 38, 0.92); color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.10);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.32), 0 1px 3px rgba(0, 0, 0, 0.2);
    font: 500 12px/1.4 "Segoe UI", "Microsoft JhengHei UI", system-ui, sans-serif;
  }}
  .stage.below .pp-panel {{ margin-bottom: 0; margin-top: 9px; }}
  body[data-panel="skin"] #skin-panel, body[data-panel="help"] #help-panel, body[data-panel="quit"] #quit-panel {{ display: block; }}
  body[data-panel] #pet-bubble {{ display: none !important; }}
  .pp-panel .row {{ display: flex; gap: 6px; align-items: center; }}
  .pp-panel .row + .row, .pp-panel .row + .text {{ margin-top: 6px; }}
  .pp-panel .title {{ flex: 1; min-width: 0; font-weight: 700; padding-left: 2px; }}
  .pp-panel .text {{ white-space: pre-line; padding: 0 2px; color: rgba(255, 255, 255, 0.88); }}
  .pp-panel .keys {{ margin-top: 6px; border-collapse: collapse; width: 100%; color: rgba(255, 255, 255, 0.88); }}
  .pp-panel .keys td {{ padding: 1px 2px; vertical-align: top; }}
  .pp-panel .keys td:first-child {{ white-space: nowrap; padding-right: 10px; font-weight: 700; color: #fff; }}
  .pp-panel input, .pp-panel button {{
    box-sizing: border-box; height: 26px; border-radius: 7px; font: inherit; color: #fff; outline: none;
    border: 1px solid rgba(255, 255, 255, 0.16); background: rgba(255, 255, 255, 0.08);
  }}
  .pp-panel input {{ flex: 1; min-width: 0; padding: 0 8px; cursor: text !important; }}
  .pp-panel input:focus {{ border-color: rgba(150, 175, 255, 0.85); }}
  .pp-panel input::placeholder {{ color: rgba(255, 255, 255, 0.45); }}
  .pp-panel button {{ padding: 0 10px; cursor: pointer !important; }}
  .pp-panel button:hover {{ background: rgba(255, 255, 255, 0.18); }}
  #skin-import, #skin-petdex, #pet-quit {{ flex: 1; }}
</style>
</head>
<body>
<div class="stage"><div id="skin-panel" class="pp-panel"><div class="row"><input id="skin-input" type="text" spellcheck="false" autocomplete="off"></div><div class="row"><button id="skin-petdex"></button><button id="skin-import"></button></div></div><div id="help-panel" class="pp-panel"><div class="row"><span id="help-title" class="title"></span></div><table id="help-keys" class="keys"></table></div><div id="quit-panel" class="pp-panel"><div class="row"><span id="quit-title" class="title"></span></div><div class="row"><button id="pet-quit"></button></div></div><div class="pet" id="pet" data-state="idle"><canvas id="pet-canvas" width="192" height="208" style="display:block;width:100%;height:100%;pointer-events:none;image-rendering:pixelated"></canvas><div id="minis"><canvas width="192" height="208"></canvas><canvas width="192" height="208"></canvas><canvas width="192" height="208"></canvas></div></div></div>
{demo_label}
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
// 測試版（debug build）把除錯訊息送給主程式記到 pet-debug.log；正式版不送
var PET_DEBUG = {pet_debug};
function dbg(msg) {{ if (PET_DEBUG) window.ipc.postMessage(JSON.stringify({{type:'debug', msg: String(msg)}})); }}
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
function drawCell(ctx, cv, c, r) {{
  ctx.clearRect(0, 0, cv.width, cv.height);
  ctx.drawImage(spriteImg, c * cellW, r * cellH, cellW, cellH, 0, 0, cv.width, cv.height);
}}
function showFrame(c, r) {{
  curFrame = {{ c: c, r: r }};
  if (!spriteImg) return;
  drawCell(petCtx, petCanvas, c, r);
  minisMainFrame(c, r);
}}
{minis_js}
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

// --- 面板：F1 換外觀、F4 快捷鍵說明、Esc 離開。同一個鍵再按一次關閉（沒有 ✕）；一次只開一個 ---
var skinInput = document.getElementById('skin-input');
function setText(id, group, key) {{ document.getElementById(id).textContent = M(group, key); }}
function setTitle(id, group, key) {{ document.getElementById(id).title = M(group, key); }}
skinInput.placeholder = M('panel', 'placeholder');
setText('skin-import', 'panel', 'import');
setText('skin-petdex', 'panel', 'petdex');
setText('help-title', 'help', 'title');
// 快捷鍵說明表：messages.json 的 help.keys（按鍵 → 說明，依檔案順序）
(function() {{
  var keys = (MSG.help && MSG.help.keys) || {{}}, table = document.getElementById('help-keys');
  Object.keys(keys).forEach(function(k) {{
    var tr = table.insertRow(), a = tr.insertCell(), b = tr.insertCell();
    a.textContent = k; b.textContent = String(keys[k]);
  }});
}})();
setText('quit-title', 'panel', 'quit_title');
setText('pet-quit', 'panel', 'quit');
setTitle('pet-quit', 'panel', 'quit_hint');
function currentPanel() {{ return document.body.dataset.panel || null; }}
// name：'skin' / 'help' / 'quit'；null 關閉。已開著同一個就關閉
window.togglePanel = function(name) {{
  var open = name && currentPanel() !== name ? name : null;
  if (open) document.body.dataset.panel = open; else delete document.body.dataset.panel;
  if (open === 'skin') {{ skinInput.value = ''; skinInput.focus(); }} else skinInput.blur();
  dbg('面板 ' + (open || '關'));
  window.syncWindowSize();
  reportHitRects();
}};
function closePanel() {{ window.togglePanel(null); }}
skinInput.addEventListener('keydown', function(e) {{
  if (e.isComposing || e.keyCode === 229) return; // 輸入法選字中
  if (e.key !== 'Enter') return;
  e.preventDefault();
  var v = skinInput.value.trim();
  dbg('輸入框按 Enter，內容=[' + v + ']');
  if (!v) return;
  window.ipc.postMessage(JSON.stringify({{type:'setSkin', value: v}}));
  closePanel();
}});
document.getElementById('skin-import').addEventListener('click', function() {{
  window.ipc.postMessage(JSON.stringify({{type:'importSkin'}}));
  closePanel();
}});
// 用預設瀏覽器開 Petdex 網站（網址固定在主程式，不從前端帶）
document.getElementById('skin-petdex').addEventListener('click', function() {{
  window.ipc.postMessage(JSON.stringify({{type:'openPetdex'}}));
  closePanel();
}});
// 讓寵物離開：和 /pawpeek:quit 相同（下次開新對話才會再出現）；Copilot 沒有指令，靠這個關
document.getElementById('pet-quit').addEventListener('click', function() {{
  closePanel();
  window.ipc.postMessage(JSON.stringify({{type:'quit'}}));
}});

// --- 鍵盤（點一下寵物取得焦點後可用）---
// F1 / F4 / Esc：開關面板（Esc 在其他面板開著時是關閉它）；[ / ]：縮小 / 放大，範圍 0.5～1.5（在面板輸入框打字時不縮放）
document.addEventListener('keydown', function(e) {{
  dbg('按鍵 ' + e.key + '（目標 ' + (e.target.id || e.target.tagName) + '）');
  if (e.isComposing || e.keyCode === 229) return; // 輸入法選字中（Esc 是取消選字）
  // 重新整理會讓寵物狀態歸零：舊版 WebView2 關不掉瀏覽器快捷鍵時在這裡擋
  if (e.key === 'F5' || ((e.ctrlKey || e.metaKey) && (e.key === 'r' || e.key === 'R'))) {{ e.preventDefault(); return; }}
  if (e.key === 'F1') {{ e.preventDefault(); window.togglePanel('skin'); return; }}
  if (e.key === 'F3') {{ e.preventDefault(); window.toggleMinis(); return; }}
  if (e.key === 'F4') {{ e.preventDefault(); window.togglePanel('help'); return; }}
  if (e.key === 'Escape') {{
    e.preventDefault();
    var p = currentPanel();
    window.togglePanel(p && p !== 'quit' ? null : 'quit');
    return;
  }}
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  if (e.target === skinInput) return;
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
        bubble_css=crate::bubble::CSS, bubble_js=crate::bubble::JS, controller_js=CONTROLLER_JS, minis_js=MINIS_JS, demo_label=demo_label,
        pet_debug=cfg!(debug_assertions))
}

/// F3 小寵物（docs/BEHAVIOR.md §3.6）。需要頁面先定義 `STATES`、`buildFrames`、`drawCell`、`spriteImg`、`curFrame`、`dbg`；
/// 對外：`minisMainFrame(c, r)`（主體每格呼叫）、`toggleMinis()`、`setMinisFollow(on)`、`minisAction(ms)`
const MINIS_JS: &str = r#"
var miniCanvases = Array.from(document.querySelectorAll('#minis canvas'));
var minis = miniCanvases.map(function(cv) { return { cv: cv, ctx: cv.getContext('2d'), timer: null, actTimer: null }; });
var miniFollow = false;   // true：主體在特定狀態（待機以外），三隻畫和主體同一格
var MINI_ACTIONS = ['jumping', 'waving', 'review', 'waiting'];
var miniShape = null;     // 上次的排列（'up' ▲ / 'down' ▼），下次換另一種
function minisOn() { return document.body.classList.contains('minis'); }
function minisMainFrame(c, r) {
  if (!minisOn() || !miniFollow || !spriteImg) return;
  minis.forEach(function(m) { drawCell(m.ctx, m.cv, c, r); });
}
function miniStop(m) { clearTimeout(m.timer); m.timer = null; clearTimeout(m.actTimer); m.actTimer = null; }
// 各自播放：從隨機一格開始，三隻不同步
function miniPlay(m, state) {
  clearTimeout(m.timer); m.timer = null;
  var frames = buildFrames(STATES[state] || STATES.idle);
  var i = Math.floor(Math.random() * frames.length);
  function draw() { if (spriteImg) drawCell(m.ctx, m.cv, frames[i].c, frames[i].r); }
  draw();
  if (frames.length < 2) return;
  (function tick() {
    m.timer = setTimeout(function() { i = (i + 1) % frames.length; draw(); tick(); }, frames[i].d);
  })();
}
function miniAct(m, action, ms) {
  miniPlay(m, action);
  clearTimeout(m.actTimer);
  m.actTimer = setTimeout(function() { m.actTimer = null; if (!miniFollow) miniPlay(m, 'idle'); }, ms);
}
function shuffled(a) {
  a = a.slice();
  for (var i = a.length - 1; i > 0; i--) { var j = Math.floor(Math.random() * (i + 1)), t = a[i]; a[i] = a[j]; a[j] = t; }
  return a;
}
function minisFree() { minis.forEach(function(m) { miniStop(m); miniPlay(m, 'idle'); }); }
// 主體換狀態（enterMode）：待機各自動，其他狀態跟主體一致
window.setMinisFollow = function(on) {
  miniFollow = !!on;
  if (!minisOn()) return;
  if (miniFollow) { minis.forEach(miniStop); minisMainFrame(curFrame.c, curFrame.r); } else minisFree();
};
// 主體待機時做小動作（自言自語、單擊）：三隻各換一個不同的動作，同樣長度
window.minisAction = function(ms) {
  if (!minisOn() || miniFollow) return;
  var acts = shuffled(MINI_ACTIONS);
  minis.forEach(function(m, i) { miniAct(m, acts[i % acts.length], ms); });
};
// 待機時偶爾自己做個小動作（每 4 秒每隻 15%，2～4 秒）
setInterval(function() {
  if (!minisOn() || miniFollow) return;
  minis.forEach(function(m) {
    if (!m.actTimer && Math.random() < 0.15) miniAct(m, MINI_ACTIONS[Math.floor(Math.random() * MINI_ACTIONS.length)], 2000 + Math.random() * 2000);
  });
}, 4000);
// 站位：寵物右下角排成三角形，▲ / ▼ 輪流（第一次隨機）；同排中心距 32%、上下排差 14%（寬 28%，上下排斜向稍微重疊）；各 ±4% 隨機；下排疊在前面
function placeMinis() {
  miniShape = miniShape === 'up' ? 'down' : miniShape === 'down' ? 'up' : (Math.random() < 0.5 ? 'up' : 'down');
  var CX = 83, DX = 16, HI = 64, LO = 78, W = 28;
  var spots = miniShape === 'up' ? [[CX, HI], [CX - DX, LO], [CX + DX, LO]] : [[CX - DX, HI], [CX + DX, HI], [CX, LO]];
  function j() { return Math.random() * 8 - 4; }
  spots.forEach(function(p, i) {
    var cv = miniCanvases[i];
    cv.style.left = (p[0] - W / 2 + j()) + '%';
    cv.style.top = (p[1] + j()) + '%';
    cv.style.zIndex = p[1] === LO ? 2 : 1;
  });
}
window.toggleMinis = function() {
  var on = !minisOn();
  document.body.classList.toggle('minis', on);
  if (on) {
    placeMinis();
    if (miniFollow) minisMainFrame(curFrame.c, curFrame.r); else minisFree();
  } else minis.forEach(miniStop);
  // 登場 / 退場說一句（暫時台詞：特殊泡泡顯示中不說）
  speech.say(pickMsg((MSG.minis || {})[on ? 'on' : 'off']), 3000);
  dbg('小寵物 ' + (on ? '開 ' + miniShape : '關'));
  reportHitRects();
};
"#;

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
function setFlip(on) {
  var t = on ? 'scaleX(-1)' : '';
  petCanvas.style.transform = t;
  miniCanvases.forEach(function(cv) { cv.style.transform = t; }); // 小寵物各自翻轉（站位不鏡像）
}
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
  window.minisAction(ms); // 小寵物各自換不同的動作（§3.6）
  clearTimeout(actionTimer);
  actionTimer = setTimeout(function() { actionTimer = null; play(modeAnim()); }, ms);
}

// 持久台詞前面加上資料夾名稱的狀態（等待確認、出錯）；完成（工作中 → 待機）的那一句另外在 onHookState 加
var FOLDER_STATES = { waiting: 1, review: 1, failed: 1 };
// 「📁 資料夾」放在第一行；沒有資料夾名稱或 folder.tag 清空就原樣
function withFolder(text, folder) {
  var tag = folder ? fmt(M('folder', 'tag'), {folder: folder}) : '';
  return tag && text ? tag + '\n' + text : text;
}

// 進入狀態：自言自語重新計時、換動畫、換台詞
//   工作中等狀態與拖曳：持久台詞，一直顯示到狀態改變
//   待機：沒有持久台詞，進入時說一句暫時台詞（entry）
function enterMode(m, entry, folder) {
  mode = m; modeSince = Date.now(); lastChatter = modeSince;
  dragDir = 0; dragAccX = 0; setFlip(false);
  clearTimeout(actionTimer); actionTimer = null;
  window.setMinisFollow(m !== 'idle'); // 小寵物：待機各自動，其他狀態跟主體一致（§3.6）
  play(modeAnim());
  var base = m === 'dragging' ? pickMsg(chatterPool('dragging'))
           : m === 'idle' ? '' : M('state_bubble', STATE_BUBBLE[m]);
  if (FOLDER_STATES[m]) base = withFolder(base, folder);
  speech.enter(base, entry);
  if (m === 'idle' && pendingUpdate) {
    var t = pendingUpdate; pendingUpdate = null;
    speech.special(t, 0, true); // 顯示到狀態改變；期間待機的自言自語暫停
  }
}

// 主程式通知 Claude Code 狀態改變（prev 為 null 表示剛啟動；folder 是造成這個狀態的對話的資料夾名稱，可能為 null）
window.onHookState = function(s, prev, folder) {
  hookState = s;
  window.__realState = s;
  window.__stateLabel = stateLabel(s);
  // 轉回待機時仍按著寵物：直接接上拖曳
  if (s === 'idle' && dragHeld) return enterMode('dragging');
  var entry = null;
  if (s === 'idle' && prev) entry = prev === 'running' ? withFolder(pickMsg(MSG.done), folder) : pickMsg(chatterPool('idle'));
  enterMode(s, entry, folder);
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
// 可拖曳 / 可點範圍 = 視窗形狀（寵物方框 + 顯示中的泡泡，見 reportHitRects），兩者一律相同
function inHitArea(e) {
  // 面板（F1 / F4 / Esc）裡不算寵物（不拖曳、不觸發單擊 / 雙擊）
  if (e.target.closest && e.target.closest('.pp-panel')) return false;
  var rs = window.__hitRects;
  if (!rs) return e.target === pet || pet.contains(e.target);
  for (var i = 0; i < rs.length; i++) {
    var r = rs[i];
    if (e.clientX >= r[0] && e.clientX < r[0] + r[2] && e.clientY >= r[1] && e.clientY < r[1] + r[3]) return true;
  }
  return false;
}
document.body.addEventListener('mousedown', function(e) {
  if (e.button !== 0) return;
  if (!inHitArea(e)) return;
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

// --- 可點範圍：視窗形狀 = 寵物方框 + 顯示中的泡泡（看得到才畫得出來），其餘穿透到底下的視窗 ---
// 同一份矩形也是可拖曳 / 可點範圍（inHitArea）
window.__lastHit = '';
function reportHitRects() {
  var r = pet.getBoundingClientRect();
  var rects = [[r.left, r.top, r.width, r.height]];
  var b = document.getElementById('pet-bubble');
  if (b && b.offsetWidth > 0) {
    var q = b.getBoundingClientRect(), P = 9; // 含尾巴與陰影
    rects.push([q.left - P, q.top - P, q.width + P * 2, q.height + P * 2]);
  }
  // F3 小寵物超出寵物方框的部分也要算進視窗形狀才畫得出來（同時也是可點 / 可拖曳範圍）
  if (minisOn()) miniCanvases.forEach(function(cv) {
    var m = cv.getBoundingClientRect();
    rects.push([m.left, m.top, m.width, m.height]);
  });
  document.querySelectorAll('.pp-panel').forEach(function(sp) {
    if (!sp.offsetWidth) return;
    var s = sp.getBoundingClientRect(), S = 4; // 含陰影
    rects.push([s.left - S, s.top - S, s.width + S * 2, s.height + S * 2]);
  });
  rects = rects.map(function(a) { return a.map(function(n) { return Math.round(n); }); });
  window.__hitRects = rects;
  var s = JSON.stringify(rects);
  if (s === window.__lastHit) return;
  window.__lastHit = s;
  window.ipc.postMessage(JSON.stringify({type:'hitRects', rects: rects}));
}
window.addEventListener('resize', reportHitRects);
if (window.ResizeObserver) {
  var ro = new ResizeObserver(reportHitRects);
  ro.observe(pet);
  var bb = document.getElementById('pet-bubble');
  if (bb) ro.observe(bb);
}
setInterval(reportHitRects, 100);
reportHitRects();

// --- 測試版標示：DEMO 放在寵物方框內的底部（不擴大可點範圍；正式版沒有這個元素） ---
(function() {
  var d = document.getElementById('demo-label');
  if (!d) return;
  function place() {
    var r = pet.getBoundingClientRect();
    d.style.left = (r.left + r.width / 2) + 'px';
    d.style.top = (r.bottom - d.offsetHeight - 2) + 'px';
  }
  window.addEventListener('resize', place);
  if (window.ResizeObserver) new ResizeObserver(place).observe(pet);
  setInterval(place, 100);
  place();
})();

// --- 單擊：依目前狀態說一句自言自語（待機時加一個小動作） ---
// 等 CLICK_WAIT_MS 確定不是雙擊才算單擊（雙擊時瀏覽器會先送兩次 click）
var CLICK_WAIT_MS = 250;
var clickBusy = false, clickTimer = null;
document.addEventListener('click', function(e) {
  if (!inHitArea(e)) return;
  if (skipClick || wasDrag) { skipClick = false; return; }
  if (clickTimer) return; // 第二下：交給 dblclick
  clickTimer = setTimeout(function() { clickTimer = null; singleClick(); }, CLICK_WAIT_MS);
});
function singleClick() {
  if (clickBusy) return;
  // 等你確認：點一下清掉這則提示（主程式換下一則或回到其他狀態），不說話；其他狀態照常說一句
  if (mode === 'waiting') {
    window.ipc.postMessage(JSON.stringify({type:'dismissConfirm'}));
    return;
  }
  var m = (mode === 'dragging' || !mode) ? 'idle' : mode;
  if (!speech.say(pickMsg(chatterPool(m)), CHATTER_MS)) return; // 特殊狀態期間不說
  if (m === 'idle') {
    clickBusy = true;
    playAction(CLICK_ACTIONS[Math.floor(Math.random() * CLICK_ACTIONS.length)], 2000);
    setTimeout(function() { clickBusy = false; }, 2000);
  }
}

// --- 雙擊：回報對話數與狀態（特殊狀態期間不說） ---
document.addEventListener('dblclick', function(e) {
  if (!inHitArea(e)) return;
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
