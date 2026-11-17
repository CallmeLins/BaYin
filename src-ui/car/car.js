/* BaYin Car v10 — 车机运行时
 * 作用：
 *  1) 运行时自检 WebView 版本，老内核加 ba-legacy 类（关掉毛玻璃等重特效）
 *  2) 把 #root 包进「边距 + 等比缩放」框架
 *  3) 从 Rust 读显示设置并实时应用（缩放/字号/封面/四周边距）
 *  4) 监听 Rust 推送的设置变更事件，即时生效（无需重启）
 */
(function () {
  'use strict';

  var html = document.documentElement;
  html.classList.add('ba-car');

  /* ── 1. 内核自检 ─────────────────────────────────────────── */
  var m = navigator.userAgent.match(/(?:Chrome|Chromium)\/(\d+)/);
  var chromeVer = m ? parseInt(m[1], 10) : 0;
  var isLegacy = chromeVer > 0 && chromeVer < 111;
  if (isLegacy) html.classList.add('ba-legacy');
  window.__BAYIN_CAR__ = { chrome: chromeVer, legacy: isLegacy };

  /* ── 2. 桥 ───────────────────────────────────────────────── */
  function invoke(cmd, args) {
    try {
      var internals = window.__TAURI_INTERNALS__;
      if (!internals) return Promise.reject(new Error('no tauri internals'));
      if (typeof internals.invoke === 'function') return internals.invoke(cmd, args);
      if (internals.ipc && typeof internals.ipc.invoke === 'function') return internals.ipc.invoke(cmd, args);
      return Promise.reject(new Error('no invoke'));
    } catch (e) {
      return Promise.reject(e);
    }
  }
  function listen(evt, cb) {
    try {
      var internals = window.__TAURI_INTERNALS__;
      if (internals && typeof internals.invoke === 'function') {
        // Tauri 2: plugin:event|listen 需要 handler id，这里走内部 transformCallback
        var id = window.__TAURI_EVENT_PLUGIN_INTERNALS__ ? null : null;
        invoke('plugin:event|listen', { event: evt, target: { kind: 'Any' }, handler: registerHandler(cb) })
          .catch(function () {});
        return;
      }
    } catch (e) {}
  }
  var handlerSeq = 0;
  var handlers = {};
  function registerHandler(cb) {
    var id = ++handlerSeq;
    handlers[id] = cb;
    window['_' + id] = function (event) {
      try { cb(event && event.payload); } catch (e) {}
    };
    // Tauri 2 期望 handler 是 transformCallback 产生的数字
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.transformCallback === 'function') {
      return window.__TAURI_INTERNALS__.transformCallback(handlers[id]);
    }
    return id;
  }

  /* ── 3. 应用显示设置 ─────────────────────────────────────── */
  var S = { scale: 1, text: 1.3, cover: 0.85, mt: 7, mb: 0, ml: 0, mr: 0 };

  function norm(v, d) {
    var n = typeof v === 'number' ? v : parseFloat(v);
    return isFinite(n) ? n : d;
  }

  function apply(s) {
    if (!s) return;
    S.scale = norm(s.scale, S.scale);
    S.text = norm(s.textScale, S.text);
    S.cover = norm(s.coverScale, S.cover);
    S.mt = norm(s.marginTop, S.mt);
    S.mb = norm(s.marginBottom, S.mb);
    S.ml = norm(s.marginLeft, S.ml);
    S.mr = norm(s.marginRight, S.mr);

    var st = html.style;
    st.setProperty('--ba-scale', String(S.scale));
    st.setProperty('--ba-text', String(S.text));
    st.setProperty('--ba-cover', String(S.cover));

    // 注意：CSS 的 padding 百分比一律按「包含块宽度」解析，
    // 所以上下边距必须换算成像素，否则 7% 会变成 7% × 屏宽。
    var vw = window.innerWidth || html.clientWidth || 1920;
    var vh = window.innerHeight || html.clientHeight || 720;
    st.setProperty('--ba-mt-px', Math.round(vh * S.mt / 100) + 'px');
    st.setProperty('--ba-mb-px', Math.round(vh * S.mb / 100) + 'px');
    st.setProperty('--ba-ml-px', Math.round(vw * S.ml / 100) + 'px');
    st.setProperty('--ba-mr-px', Math.round(vw * S.mr / 100) + 'px');

    // 车机状态栏是覆盖层时 env() 拿不到，用原生上报的高度补
    if (S.mt === 0 && safeTop > 0) st.setProperty('--ba-safe-top', safeTop + 'px');
    else st.setProperty('--ba-safe-top', '0px');
  }

  var safeTop = 0;

  // 旋转/改窗口尺寸后重新换算像素边距
  var reflow = function () { apply(null); };
  window.addEventListener('resize', reflow);
  window.addEventListener('orientationchange', function () { setTimeout(reflow, 200); });

  /* ── 4. 包一层 边距 + 缩放 框架 ──────────────────────────── */
  function wrap() {
    var root = document.getElementById('root');
    if (!root) return false;
    if (document.querySelector('.ba-frame')) return true;
    if (!root.firstElementChild) return false;

    var frame = document.createElement('div');
    frame.className = 'ba-frame';
    var zoom = document.createElement('div');
    zoom.className = 'ba-zoom';

    var kids = [];
    while (root.firstChild) kids.push(root.removeChild(root.firstChild));
    for (var i = 0; i < kids.length; i++) zoom.appendChild(kids[i]);
    frame.appendChild(zoom);
    root.appendChild(frame);
    return true;
  }

  /* ── 5. 给封面网格打标记（用于封面尺寸调节） ─────────────── */
  function tagGrids() {
    var grids = document.querySelectorAll('[class*="grid-cols-1"][class*="md:grid-cols-3"]');
    for (var i = 0; i < grids.length; i++) grids[i].classList.add('ba-cover-grid');
  }

  /* ── 6. 启动 ─────────────────────────────────────────────── */
  var tries = 0;
  var timer = setInterval(function () {
    tries++;
    var ok = wrap();
    if (ok) {
      tagGrids();
      if (tries > 4) clearInterval(timer);
    }
    if (tries > 200) clearInterval(timer);
  }, 100);

  // 列表是动态渲染的，定期补标记（低开销）
  var lastTag = 0;
  var mo = new MutationObserver(function () {
    var now = Date.now();
    if (now - lastTag < 800) return;
    lastTag = now;
    tagGrids();
  });
  document.addEventListener('DOMContentLoaded', function () {
    var root = document.getElementById('root');
    if (root) mo.observe(root, { childList: true, subtree: true });
  });

  // 读设置
  invoke('car_get_display_settings')
    .then(apply)
    .catch(function () { apply(null); });

  // 监听设置变更
  listen('car://display-changed', apply);

  // 暴露给调试面板
  window.__BAYIN_CAR__.apply = apply;
  window.__BAYIN_CAR__.state = S;
})();