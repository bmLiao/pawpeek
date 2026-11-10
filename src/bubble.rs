//! 對話泡泡（與寵物同一個視窗）
//!
//! 泡泡出現時視窗往上長出泡泡需要的高度（寵物在螢幕上的位置不動），消失時縮回原本大小。
//! 寵物靠近螢幕頂端、上方放不下時，改成往下長、泡泡放在寵物下方。
//!
//! - `next_frame`：依新尺寸計算視窗位置（純計算，可測試）
//! - `CSS` / `JS`：嵌進寵物頁面的泡泡樣式與邏輯（深色膠囊 + 尾巴 + 打字動畫）

/// 實體像素矩形
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 視窗新的位置與大小；`below` = 泡泡在寵物下方
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub rect: Rect,
    pub below: bool,
}

/// 目前視窗中「沒有泡泡時的寵物視窗」左上角（寵物固定貼在這塊範圍內）
pub fn base_origin(win: Rect, base: (i32, i32), below: bool) -> (i32, i32) {
    let x = win.x + (win.w - base.0) / 2;
    let y = if below { win.y } else { win.y + win.h - base.1 };
    (x, y)
}

/// 計算新視窗位置。
/// - `win`：目前視窗；`old_base` / `below`：目前沒有泡泡時的大小與泡泡方向
/// - `new_base`：新的寵物視窗大小（縮放可能改變）；`new_size`：要變成的視窗大小
/// - 寵物以底部中央為基準（縮放時往上長）；有泡泡時優先往上長，超出 `screen_top` 就改往下長
pub fn next_frame(
    win: Rect,
    old_base: (i32, i32),
    below: bool,
    new_base: (i32, i32),
    new_size: (i32, i32),
    screen_top: i32,
) -> Frame {
    let (ox, oy) = base_origin(win, old_base, below);
    // 寵物視窗本身（縮放時以底部中央為基準）
    let bx = ox + (old_base.0 - new_base.0) / 2;
    let by = oy + old_base.1 - new_base.1;
    let x = bx - (new_size.0 - new_base.0) / 2;
    let has_bubble = new_size.1 > new_base.1;
    let up_y = by + new_base.1 - new_size.1;
    let (y, below) = if !has_bubble {
        (by, false)
    } else if up_y >= screen_top {
        (up_y, false)
    } else {
        (by, true)
    };
    Frame { rect: Rect { x, y, w: new_size.0, h: new_size.1 }, below }
}

/// 寵物「回家」的位置（啟動、按 c、螢幕被拔掉時）：
/// 放在作用中視窗 `active` 內側右下角，並限制在該視窗所在的螢幕內；
/// 沒有作用中視窗或它不在任何螢幕上，就放在主螢幕中央。回傳寵物視窗（不含泡泡）的左上角。
pub fn home_spot(active: Option<Rect>, monitors: &[Rect], primary: Rect, base: (i32, i32)) -> (i32, i32) {
    const MARGIN: i32 = 40;
    let contains = |m: &Rect, x: i32, y: i32| x >= m.x && y >= m.y && x < m.x + m.w && y < m.y + m.h;
    if let Some(a) = active {
        let (cx, cy) = (a.x + a.w / 2, a.y + a.h / 2);
        if let Some(m) = monitors.iter().find(|m| contains(m, cx, cy)) {
            let x = (a.x + a.w - base.0 - MARGIN).clamp(m.x, (m.x + m.w - base.0).max(m.x));
            let y = (a.y + a.h - base.1 - MARGIN).clamp(m.y, (m.y + m.h - base.1).max(m.y));
            return (x, y);
        }
    }
    (primary.x + (primary.w - base.0) / 2, primary.y + (primary.h - base.1) / 2)
}

/// 泡泡樣式（參考 agentpet：深色半透明圓角膠囊、尾巴指向寵物、陰影）
pub const CSS: &str = r#"
  .stage { flex-direction: column; justify-content: flex-end; box-sizing: border-box; }
  .stage .pet { flex: 0 0 auto; } /* 寵物圖不被泡泡擠壓 */
  .stage.below { flex-direction: column-reverse; }
  #pet-bubble {
    display: none; box-sizing: border-box; position: relative; flex: 0 0 auto;
    width: max-content; max-width: 260px; /* 不受目前視窗寬度限制，才量得到真正大小 */ padding: 7px 12px; border-radius: 12px; margin-bottom: 9px;
    background: rgba(22, 24, 38, 0.92); color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.10);
    font: 500 12px/1.4 "Segoe UI", "Microsoft JhengHei UI", system-ui, sans-serif;
    text-align: center; white-space: pre-wrap; overflow-wrap: anywhere;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.32), 0 1px 3px rgba(0, 0, 0, 0.2);
    opacity: 0; transition: opacity 0.2s ease; pointer-events: none;
  }
  .stage.below #pet-bubble { margin-bottom: 0; margin-top: 9px; }
  #pet-bubble.show { opacity: 1; }
  #pet-bubble.capsule { border-radius: 999px; }
  /* 完整文字撐出大小（看不見），打字的文字疊在同一格，打字時泡泡不跳動 */
  #pet-bubble .cell { display: grid; }
  #pet-bubble .ghost, #pet-bubble .typed { grid-area: 1 / 1; }
  #pet-bubble .ghost { visibility: hidden; }
  #pet-bubble .tail { position: absolute; left: 50%; width: 0; height: 0; border: 7px solid transparent; transform: translateX(-50%); }
  .stage:not(.below) #pet-bubble .tail { bottom: -7px; border-top-color: rgba(22, 24, 38, 0.92); border-bottom: 0; }
  .stage.below #pet-bubble .tail { top: -7px; border-bottom-color: rgba(22, 24, 38, 0.92); border-top: 0; }
  #pet-bubble .typed.shimmer {
    background: linear-gradient(100deg, #fff 40%, rgba(255,255,255,0.45) 50%, #fff 60%);
    background-size: 220% 100%; -webkit-background-clip: text; background-clip: text; color: transparent;
    animation: bubble-shimmer 2.5s linear infinite;
  }
  @keyframes bubble-shimmer { from { background-position: 120% 0; } to { background-position: -120% 0; } }
"#;

/// 泡泡邏輯。需要頁面先定義 `pet`、`stageEl`、`window.__baseSize`（{w,h} 寵物視窗邏輯大小）。
/// 對外介面：`setBubble(text, ms, persist)`、`showTransientBubble(text, ms)`、
/// `setBubbleSide(below)`（主程式通知泡泡改到寵物下方）、`syncWindowSize()`（縮放後重算視窗大小）
pub const JS: &str = r#"
(function() {
  var ERASE_MS = 20, TYPE_MS = 45, DOT_MS = 400, DOTS = ['.', '..', '...'];
  var el = document.createElement('div');
  el.id = 'pet-bubble';
  el.innerHTML = '<div class="cell"><span class="ghost"></span><span class="typed"></span></div><div class="tail"></div>';
  stageEl.insertBefore(el, pet);
  var ghost = el.querySelector('.ghost'), typed = el.querySelector('.typed');
  var visible = false, target = '', timer = null, hideTimer = null;

  // 視窗大小：寵物 + 一律預留泡泡空間（最寬 260px、約 4 行高）。
  // 泡泡消失只隱藏、不縮視窗：縮放視窗時 WebView2 會晚一兩幀才重繪，寵物會錯位閃一下。
  // 文字超出預留空間才放大，之後不再縮回。（主程式負責讓寵物在螢幕上不動）
  var RESERVE_W = 276, RESERVE_H = 84, reserveW = 0, reserveH = 0, lastSize = '';
  window.syncWindowSize = function() {
    var b = window.__baseSize, s = window.__petScale || 1;
    var bw = Math.ceil(RESERVE_W * s), bh = Math.ceil(RESERVE_H * s);
    if (visible) {
      var r = el.getBoundingClientRect();
      reserveW = Math.max(reserveW, Math.ceil(r.width) + 16);
      reserveH = Math.max(reserveH, Math.ceil(r.height));
    }
    var w = Math.max(b.w, bw, reserveW);
    var h = b.h + Math.max(bh, reserveH) + Math.ceil(12 * s);
    var key = w + 'x' + h + '/' + b.w + 'x' + b.h;
    if (key === lastSize) return;
    lastSize = key;
    window.ipc.postMessage(JSON.stringify({type:'resize', w: w, h: h, baseW: b.w, baseH: b.h}));
  };
  // 縮放後預留空間依新比例重算
  window.resetBubbleReserve = function() { reserveW = 0; reserveH = 0; };
  window.setBubbleSide = function(below) { stageEl.classList.toggle('below', !!below); };

  function stop() { if (timer) { clearInterval(timer); timer = null; } typed.classList.remove('shimmer'); }
  function splitDots(s) {
    if (/(\.\.\.|…)$/.test(s)) return { base: s.replace(/(\.\.\.|…)$/, ''), dots: true };
    return { base: s, dots: false };
  }
  function typeIn(full) {
    var e = splitDots(full), chars = Array.from(e.base), i = 0, shown = '';
    typed.textContent = '';
    timer = setInterval(function() {
      if (i < chars.length) { shown += chars[i++]; typed.textContent = shown; return; }
      clearInterval(timer); timer = null;
      if (e.dots) {
        var f = 0; typed.textContent = e.base + DOTS[0];
        timer = setInterval(function() { f = (f + 1) % DOTS.length; typed.textContent = e.base + DOTS[f]; }, DOT_MS);
      } else {
        typed.classList.add('shimmer');
      }
    }, TYPE_MS);
  }
  function show(text) {
    clearTimeout(hideTimer);
    if (text === target && visible) return;
    var wasVisible = visible;
    target = text; visible = true; stop();
    var cur = wasVisible ? Array.from(typed.textContent) : [];
    if (!wasVisible) typed.textContent = '';
    ghost.textContent = splitDots(text).dots ? splitDots(text).base + '...' : text;
    el.style.display = 'block';
    el.classList.toggle('capsule', el.getBoundingClientRect().height <= 34 * (window.__petScale || 1));
    window.syncWindowSize();
    requestAnimationFrame(function() { el.classList.add('show'); });
    if (!cur.length) return typeIn(text);
    // 先刪掉舊字再打新字
    timer = setInterval(function() {
      cur.pop(); typed.textContent = cur.join('');
      if (!cur.length) { clearInterval(timer); timer = null; typeIn(text); }
    }, ERASE_MS);
  }
  function hide() {
    if (!visible) return;
    visible = false; target = ''; stop();
    el.classList.remove('show');
    hideTimer = setTimeout(function() {
      el.style.display = 'none'; typed.textContent = ''; ghost.textContent = '';
    }, 220);
  }

  var persistent = '', backTimer = null;
  function restoreLater(ms) {
    var saved = persistent;
    backTimer = setTimeout(function() { if (saved) show(saved); else hide(); }, ms);
  }
  window.setBubble = function(text, ms, persist) {
    clearTimeout(backTimer);
    if (persist) persistent = text || '';
    if (!text) { persistent = ''; hide(); return; }
    show(text);
    if (!persist && ms) restoreLater(ms);
  };
  // 用户交互触发的瞬态气泡：自动恢复底层的持久气泡
  window.showTransientBubble = function(text, ms) {
    clearTimeout(backTimer);
    if (!text) return;
    show(text);
    restoreLater(ms || 3000);
  };
})();
"#;

#[cfg(test)]
mod tests {
    use super::*;

    const PRIMARY: Rect = Rect { x: 0, y: 0, w: 1920, h: 1080 };
    const LEFT: Rect = Rect { x: -1920, y: 0, w: 1920, h: 1080 };

    #[test]
    fn home_inside_active_window_bottom_right() {
        let term = Rect { x: 100, y: 100, w: 800, h: 600 };
        assert_eq!(home_spot(Some(term), &[PRIMARY, LEFT], PRIMARY, (180, 220)), (100 + 800 - 180 - 40, 100 + 600 - 220 - 40));
    }

    #[test]
    fn home_on_second_monitor_when_window_there() {
        let term = Rect { x: -1900, y: 50, w: 1000, h: 900 };
        assert_eq!(home_spot(Some(term), &[PRIMARY, LEFT], PRIMARY, (180, 220)), (-1900 + 1000 - 220, 50 + 900 - 260));
    }

    #[test]
    fn home_clamped_to_monitor() {
        // 視窗比寵物小、或超出螢幕邊：仍留在螢幕內
        let tiny = Rect { x: 1850, y: 1000, w: 100, h: 100 };
        let (x, y) = home_spot(Some(tiny), &[PRIMARY], PRIMARY, (180, 220));
        assert!(x >= 0 && x + 180 <= 1920 && y >= 0 && y + 220 <= 1080, "({x},{y})");
    }

    #[test]
    fn home_falls_back_to_primary_center() {
        assert_eq!(home_spot(None, &[PRIMARY], PRIMARY, (180, 220)), (870, 430));
        // 作用中視窗在已拔掉的螢幕上
        let lost = Rect { x: -1500, y: 100, w: 800, h: 600 };
        assert_eq!(home_spot(Some(lost), &[PRIMARY], PRIMARY, (180, 220)), (870, 430));
    }

    const BASE: (i32, i32) = (140, 180);

    #[test]
    fn bubble_grows_upward_pet_stays() {
        let win = Rect { x: 1000, y: 500, w: 140, h: 180 };
        let f = next_frame(win, BASE, false, BASE, (280, 250), 0);
        assert_eq!(f, Frame { rect: Rect { x: 930, y: 430, w: 280, h: 250 }, below: false });
        // 寵物視窗位置沒變
        assert_eq!(base_origin(f.rect, BASE, f.below), (1000, 500));
    }

    #[test]
    fn bubble_goes_below_near_screen_top() {
        let win = Rect { x: 1000, y: 20, w: 140, h: 180 };
        let f = next_frame(win, BASE, false, BASE, (280, 250), 0);
        assert!(f.below);
        assert_eq!(f.rect, Rect { x: 930, y: 20, w: 280, h: 250 });
        assert_eq!(base_origin(f.rect, BASE, true), (1000, 20));
    }

    #[test]
    fn shrinks_back_from_above_and_below() {
        let up = Rect { x: 930, y: 430, w: 280, h: 250 };
        assert_eq!(next_frame(up, BASE, false, BASE, BASE, 0).rect, Rect { x: 1000, y: 500, w: 140, h: 180 });
        let down = Rect { x: 930, y: 20, w: 280, h: 250 };
        let f = next_frame(down, BASE, true, BASE, BASE, 0);
        assert_eq!(f, Frame { rect: Rect { x: 1000, y: 20, w: 140, h: 180 }, below: false });
    }

    #[test]
    fn scale_change_keeps_bottom_center() {
        let win = Rect { x: 1000, y: 500, w: 140, h: 180 };
        let f = next_frame(win, BASE, false, (180, 220), (180, 220), 0);
        assert_eq!(f.rect, Rect { x: 980, y: 460, w: 180, h: 220 });
    }

    #[test]
    fn text_change_while_above_keeps_pet() {
        let win = Rect { x: 930, y: 430, w: 280, h: 250 };
        let f = next_frame(win, BASE, false, BASE, (200, 230), 0);
        assert_eq!(base_origin(f.rect, BASE, f.below), (1000, 500));
    }
}
