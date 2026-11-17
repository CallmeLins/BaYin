/* BaYin Car — 【屏幕缩放与边距调整】面板
 *
 * 上游 UI 没有这一页，车机分支用注入层提供：
 *   - 缩放 70%~140%、字号 80%~200%、封面 60%~130%、四周边距 0%~30%
 *   - 每一项点一次即时生效（写入 Rust 端 display.json 并广播到 Web 层）
 *   - 「恢复默认」= 回到车机预设（缩放100% / 字号130% / 封面85% / 上边距7%）
 */
(function () {
  'use strict';

  var DEFAULT_SETTINGS = {
    scale: 1.0,
    textScale: 1.3,
    coverScale: 0.85,
    marginTop: 7,
    marginBottom: 0,
    marginLeft: 0,
    marginRight: 0
  };

  var RANGES = {
    scale: [0.70, 1.40, 0.05],
    textScale: [0.80, 2.00, 0.05],
    coverScale: [0.60, 1.30, 0.05],
    marginTop: [0, 30, 1],
    marginBottom: [0, 30, 1],
    marginLeft: [0, 30, 1],
    marginRight: [0, 30, 1]
  };

  var LABELS = [
    ['scale', '缩放', 'pct'],
    ['textScale', '字号', 'pct'],
    ['coverScale', '封面尺寸', 'pct'],
    ['marginTop', '屏幕上边距', 'pcti'],
    ['marginBottom', '屏幕下边距', 'pcti'],
    ['marginLeft', '屏幕左边距', 'pcti'],
    ['marginRight', '屏幕右边距', 'pcti']
  ];

  function invoke(cmd, args) {
    try {
      var it = window.__TAURI_INTERNALS__;
      if (it && typeof it.invoke === 'function') return it.invoke(cmd, args);
    } catch (e) {}
    return Promise.reject(new Error('no tauri'));
  }

  function fmt(key, v) {
    var mode = key.indexOf('margin') === 0 ? 'pcti' : 'pct';
    var n = Math.round(v * 100) / 100;
    if (mode === 'pcti') return Math.round(n) + '%';
    return Math.round(n * 100) + '%';
  }

  function clamp(key, v) {
    var r = RANGES[key];
    var q = Math.round(v / r[2]) * r[2];
    return Math.min(r[1], Math.max(r[0], Math.round(q * 100) / 100));
  }

  var state = {};
  Object.keys(DEFAULT_SETTINGS).forEach(function (k) { state[k] = DEFAULT_SETTINGS[k]; });

  var mask = null;
  var rows = {};

  function buildPanel() {
    mask = document.createElement('div');
    mask.className = 'ba-panel-mask';

    var panel = document.createElement('div');
    panel.className = 'ba-panel';

    var head = document.createElement('div');
    head.className = 'ba-panel-head';
    head.innerHTML =
      '<div class="ba-panel-title">屏幕缩放与边距调整</div>' +
      '<div class="ba-panel-actions">' +
      '<button class="ba-btn-text" data-act="reset">恢复默认</button>' +
      '<button class="ba-btn-close" data-act="close">×</button>' +
      '</div>';

    var note = document.createElement('div');
    note.className = 'ba-note';
    note.innerHTML =
      '请注意！这里的功能属于特殊适配操作。<br>' +
      '· 调得太大或太小可能导致页面显示异常，右上角的「恢复默认」可一键还原。<br>' +
      '· 车机顶部状态栏压住内容时，把「屏幕上边距」调到 6%~10% 即可避开。<br>' +
      '· 设置即时生效，无需重启应用。';

    var groups = document.createElement('div');
    groups.className = 'ba-groups';

    var left = document.createElement('div');
    left.className = 'ba-group';
    left.innerHTML = '<div class="ba-group-title">缩放与字号</div>';
    var right = document.createElement('div');
    right.className = 'ba-group';
    right.innerHTML = '<div class="ba-group-title">四周边距</div>';

    LABELS.forEach(function (item, idx) {
      var key = item[0], label = item[1];
      var row = document.createElement('div');
      row.className = 'ba-row';
      row.innerHTML =
        '<span class="ba-row-label">' + label + '</span>' +
        '<button class="ba-step" data-key="' + key + '" data-dir="-1">−</button>' +
        '<span class="ba-row-value" data-val="' + key + '">–</span>' +
        '<button class="ba-step" data-key="' + key + '" data-dir="1">+</button>';
      (idx < 3 ? left : right).appendChild(row);
      rows[key] = row;
    });

    groups.appendChild(left);
    groups.appendChild(right);
    panel.appendChild(head);
    panel.appendChild(note);
    panel.appendChild(groups);
    mask.appendChild(panel);
    document.body.appendChild(mask);

    mask.addEventListener('click', function (e) {
      var t = e.target;
      if (t === mask) { close(); return; }
      var act = t.getAttribute && t.getAttribute('data-act');
      if (act === 'close') { close(); return; }
      if (act === 'reset') { reset(); return; }
      var key = t.getAttribute && t.getAttribute('data-key');
      var dir = t.getAttribute && t.getAttribute('data-dir');
      if (key && dir) step(key, parseInt(dir, 10));
    });

    // 注入 panel.css（避免影响 index.html 结构）
    var link = document.createElement('link');
    link.rel = 'stylesheet';
    link.href = '/car/panel.css';
    document.head.appendChild(link);
  }

  function refresh() {
    LABELS.forEach(function (item) {
      var key = item[0];
      var el = mask.querySelector('[data-val="' + key + '"]');
      if (el) el.textContent = fmt(key, state[key]);
      var minus = mask.querySelector('.ba-step[data-key="' + key + '"][data-dir="-1"]');
      var plus = mask.querySelector('.ba-step[data-key="' + key + '"][data-dir="1"]');
      if (minus) minus.disabled = state[key] <= RANGES[key][0] + 1e-6;
      if (plus) plus.disabled = state[key] >= RANGES[key][1] - 1e-6;
    });
  }

  function push() {
    // 先本地即时生效，再落到 Rust（写盘 + 广播）
    if (window.__BAYIN_CAR__ && window.__BAYIN_CAR__.apply) {
      window.__BAYIN_CAR__.apply(state);
    }
    invoke('car_set_display_settings', { settings: toRustShape() }).catch(function () {});
  }

  function toRustShape() {
    return {
      scale: state.scale,
      textScale: state.textScale,
      coverScale: state.coverScale,
      marginTop: state.marginTop,
      marginBottom: state.marginBottom,
      marginLeft: state.marginLeft,
      marginRight: state.marginRight
    };
  }

  function step(key, dir) {
    state[key] = clamp(key, state[key] + dir * RANGES[key][2]);
    refresh();
    push();
  }

  function reset() {
    Object.keys(DEFAULT_SETTINGS).forEach(function (k) { state[k] = DEFAULT_SETTINGS[k]; });
    refresh();
    if (window.__BAYIN_CAR__ && window.__BAYIN_CAR__.apply) {
      window.__BAYIN_CAR__.apply(state);
    }
    invoke('car_reset_display_settings').catch(function () {});
  }

  function open() { if (mask) { refresh(); mask.classList.add('open'); } }
  function close() { if (mask) mask.classList.remove('open'); }

  // 入口：右下角悬浮按钮（车机界面上不挤占原生布局）
  function buildFab() {
    var fab = document.createElement('button');
    fab.className = 'ba-fab show';
    fab.type = 'button';
    fab.title = '屏幕缩放与边距调整';
    fab.textContent = '⚙';
    fab.addEventListener('click', open);
    document.body.appendChild(fab);
  }

  function start() {
    buildPanel();
    buildFab();
    invoke('car_get_display_settings')
      .then(function (payload) {
        var s = (payload && payload.settings) || payload;
        if (s) {
          Object.keys(DEFAULT_SETTINGS).forEach(function (k) {
            if (typeof s[k] === 'number') state[k] = s[k];
          });
        }
        refresh();
      })
      .catch(function () { refresh(); });
    buildStorageSection();
  }

  /* ── 外接存储（U 盘）扫描 ──────────────────────────────────────────
   * 方案 A（本实现）：直连路径扫描 —— 用 getExternalFilesDirs 反推挂载根，
   *   直接交给 Rust 的 walkdir 扫描并入库，零拷贝、零缓存，7.0~14 通用。
   * 方案 B（Android 11+ 直连不可读时）：请求「所有文件访问权限」后再扫。
   * 方案 C（计划中）：SAF 目录授权 + fd 读取，用于 ROM 收紧路径访问的机型。
   */
  function bridge() {
    return (window.AndroidBridge && typeof window.AndroidBridge.listStorageVolumes === 'function')
      ? window.AndroidBridge : null;
  }

  function buildStorageSection() {
    var b = bridge();
    if (!b) return;

    var box = document.createElement('div');
    box.className = 'ba-note';
    box.style.marginTop = '0';
    box.innerHTML =
      '<div style="font-weight:700;margin-bottom:8px">外接存储（U 盘）</div>' +
      '<div id="ba-store-list" style="line-height:2">正在检测…</div>' +
      '<div id="ba-store-msg" style="color:#2f7d32;margin-top:6px"></div>';

    var panel = document.querySelector('.ba-panel');
    if (panel) panel.insertBefore(box, panel.querySelector('.ba-groups'));

    refreshVolumes();

    function refreshVolumes() {
      var list = box.querySelector('#ba-store-list');
      var msg = box.querySelector('#ba-store-msg');
      var json;
      try { json = JSON.parse(b.listStorageVolumes()); } catch (e) { json = []; }

      var hasAll = false;
      try { hasAll = b.hasAllFilesAccess(); } catch (e) { hasAll = true; }

      if (!json.length) {
        list.innerHTML = '<span style="color:#c62828">未检测到可用存储卷</span>';
        return;
      }

      var html = '';
      json.forEach(function (v, i) {
        var warn = (!v.readable) ? ' <span style="color:#c62828">（不可读）</span>' : '';
        html += '<div style="display:flex;align-items:center;gap:12px;padding:4px 0">' +
          '<div style="flex:1">' + v.label +
          ' <span style="color:#77839b;font-size:13px">' + v.root + ' · 剩余 ' + Math.round(v.freeMb / 1024 * 10) / 10 + ' GB</span>' + warn + '</div>' +
          '<button class="ba-btn-text" data-scan="' + i + '">扫描</button>' +
          '</div>';
      });
      if (!hasAll) {
        html += '<div style="margin-top:8px">Android 11+ 读取 U 盘需要「所有文件访问权限」：' +
          '<button class="ba-btn-text" data-act-perm="1">去授权</button></div>';
      }
      list.innerHTML = html;

      list.querySelectorAll('[data-scan]').forEach(function (btn) {
        btn.addEventListener('click', function () {
          var v = json[parseInt(btn.getAttribute('data-scan'), 10)];
          doScan(v, msg, btn);
        });
      });
      var permBtn = list.querySelector('[data-act-perm]');
      if (permBtn) {
        permBtn.addEventListener('click', function () {
          try { b.requestAllFilesAccess(); } catch (e) {}
          msg.textContent = '授权后请返回本页点「刷新」重新检测';
          msg.style.color = '#c62828';
        });
      }
    }

    function doScan(v, msg, btn) {
      if (!v) return;
      btn.disabled = true;
      btn.textContent = '扫描中…';
      msg.style.color = '#2f7d32';
      msg.textContent = '正在扫描 ' + v.label + ' …';

      invoke('scan_local_to_db', {
        options: {
          directories: [v.root],
          mode: 'incremental',
          minDuration: 0,
          batchSize: 500,
          resume: true
        }
      }).then(function (r) {
        var added = (r && (r.added || r.Added)) || 0;
        var total = (r && (r.totalSongs || r.TotalSongs)) || 0;
        msg.textContent = '扫描完成：新增 ' + added + ' 首，库内共 ' + total + ' 首';
        btn.textContent = '扫描';
        btn.disabled = false;
      }).catch(function (e) {
        msg.style.color = '#c62828';
        msg.textContent = '扫描失败：' + (e && e.message ? e.message : e);
        btn.textContent = '重试';
        btn.disabled = false;
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', start);
  } else {
    start();
  }
})();