/* Minimal polyfills for Chrome 51~77 WebViews (Android 7~9 head units).
   Must run before the app bundle. */
(function () {
  'use strict';

  if (!window.ResizeObserver) {
    window.ResizeObserver = function (cb) {
      var targets = [];
      function check() {
        for (var i = 0; i < targets.length; i++) {
          var t = targets[i];
          var r = t.getBoundingClientRect();
          cb([{ target: t, contentRect: r }], this);
        }
      }
      this.observe = function (t) { targets.push(t); check(); };
      this.unobserve = function (t) { targets = targets.filter(function (x) { return x !== t; }); };
      this.disconnect = function () { targets = []; };
      window.addEventListener('resize', check);
      window.addEventListener('orientationchange', check);
    };
  }

  if (!window.IntersectionObserver) {
    window.IntersectionObserver = function (cb) {
      this.observe = function (t) {
        cb([{ target: t, isIntersecting: true, intersectionRatio: 1 }], this);
      };
      this.unobserve = function () {};
      this.disconnect = function () {};
    };
  }

  if (!Array.prototype.flat) {
    Array.prototype.flat = function (depth) {
      var d = depth === undefined ? 1 : depth;
      return d < 1 ? Array.prototype.slice.call(this)
        : this.reduce(function (a, v) {
            return a.concat(Array.isArray(v) ? v.flat(d - 1) : v);
          }, []);
    };
  }
  if (!Array.prototype.flatMap) {
    Array.prototype.flatMap = function (fn, thisArg) {
      return this.map(fn, thisArg).flat();
    };
  }
  if (!String.prototype.replaceAll) {
    String.prototype.replaceAll = function (s, r) {
      return this.split(s).join(r);
    };
  }
  if (!String.prototype.at) {
    String.prototype.at = function (i) {
      return this[i < 0 ? this.length + i : i];
    };
  }
  if (!Array.prototype.at) {
    Array.prototype.at = function (i) {
      return this[i < 0 ? this.length + i : i];
    };
  }
  if (!Object.fromEntries) {
    Object.fromEntries = function (it) {
      var o = {};
      for (var i = 0; i < it.length; i++) o[it[i][0]] = it[i][1];
      return o;
    };
  }
  if (!Object.entries) {
    Object.entries = function (o) {
      var r = [];
      for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) r.push([k, o[k]]);
      return r;
    };
  }
  if (!Object.values) {
    Object.values = function (o) {
      return Object.keys(o).map(function (k) { return o[k]; });
    };
  }
  if (!window.queueMicrotask) {
    window.queueMicrotask = function (fn) {
      Promise.resolve().then(fn);
    };
  }
  if (!Promise.allSettled) {
    Promise.allSettled = function (ps) {
      return Promise.all(ps.map(function (p) {
        return Promise.resolve(p).then(
          function (v) { return { status: 'fulfilled', value: v }; },
          function (e) { return { status: 'rejected', reason: e }; }
        );
      }));
    };
  }
  if (!Number.isFinite) Number.isFinite = function (v) { return typeof v === 'number' && isFinite(v); };
  if (!Math.trunc) Math.trunc = function (v) { return v < 0 ? Math.ceil(v) : Math.floor(v); };
})();