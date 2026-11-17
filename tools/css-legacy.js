/**
 * CSS 后处理：让 Tailwind v4 产物能在 Chrome 51~98 的 WebView 上渲染。
 *
 * 必须处理的三件事（lightningcss 不会做，因为它只做"值"级别的降级）：
 *   1. @layer —— Chrome < 99 不认识这个 at-rule，整块样式会被**整体丢弃**（白屏根因）。
 *      这里把每层内容原地展开（Tailwind 输出顺序 theme→base→components→utilities
 *      本身就是正确层叠顺序，展开后语义等价）。
 *   2. @property —— Chrome < 85 不支持。Tailwind 已用 @supports 提供 --tw-* 兜底，
 *      所以直接删除 @property 声明块即可。
 *   3. dvh —— Chrome < 108 不支持，降成 vh（WebView 无地址栏，两者等价）。
 */
'use strict';

/** 找到从 start（指向 '{'）开始的配对 '}'，返回其后的下标 */
function matchBrace(s, start) {
  let depth = 0;
  let quote = null;
  for (let i = start; i < s.length; i++) {
    const c = s[i];
    if (quote) {
      if (c === '\\') { i++; continue; }
      if (c === quote) quote = null;
      continue;
    }
    if (c === '"' || c === "'") { quote = c; continue; }
    if (c === '{') depth++;
    else if (c === '}') { depth--; if (depth === 0) return i + 1; }
  }
  return -1;
}

/** 展开所有 @layer <name> { ... }（保留内部内容与顺序） */
function flattenLayers(css) {
  let out = css;
  let guard = 0;
  while (guard++ < 500) {
    const m = /@layer\s+[A-Za-z0-9_.\-]+(?:[,\s]+[A-Za-z0-9_.\-]+)*\s*\{/.exec(out);
    if (!m) break;
    const open = out.indexOf('{', m.index);
    const end = matchBrace(out, open);
    if (end < 0) break;
    const inner = out.slice(open + 1, end - 1);
    out = out.slice(0, m.index) + inner + out.slice(end);
  }
  // @layer a, b;（顺序声明语句）直接删掉
  out = out.replace(/@layer\s+[A-Za-z0-9_.\-\s,]+;/g, '');
  return out;
}

/** 删除 @property --x { ... } */
function stripProperty(css) {
  let out = css;
  let guard = 0;
  while (guard++ < 1000) {
    const m = /@property\s+--[A-Za-z0-9_\-]+\s*\{/.exec(out);
    if (!m) break;
    const open = out.indexOf('{', m.index);
    const end = matchBrace(out, open);
    if (end < 0) break;
    out = out.slice(0, m.index) + out.slice(end);
  }
  return out;
}

/** dvh/svh/lvh -> vh */
function fixViewportUnits(css) {
  return css.replace(/(\d*\.?\d+)(?:dvh|svh|lvh)\b/g, '$1vh');
}

/** color-mix 在没有 @supports 保护的地方，退化为"去掉透明度"的安全值 */
function neutralizeUngatedColorMix(css) {
  // 官方产物里未加 @supports 的仅 2 处，形如 color-mix(in srgb,hsl(var(--x))50%,transparent)
  return css.replace(
    /color-mix\(\s*in\s+srgb\s*,\s*hsl\(\s*var\((--[A-Za-z0-9_\-]+)\)\s*\)\s*\d+(?:\.\d+)?%\s*,\s*transparent\s*\)/g,
    'hsla(var($1), 0.5)'
  );
}

function postprocess(css) {
  let out = css;
  out = flattenLayers(out);
  out = stripProperty(out);
  out = fixViewportUnits(out);
  out = neutralizeUngatedColorMix(out);
  return out;
}

module.exports = { postprocess };

/* 直接运行：node tools/css-legacy.js in.css out.css */
if (require.main === module) {
  const fs = require('fs');
  const [, , a, b] = process.argv;
  if (!a || !b) {
    console.error('usage: node css-legacy.js <in.css> <out.css>');
    process.exit(1);
  }
  const res = postprocess(fs.readFileSync(a, 'utf8'));
  fs.writeFileSync(b, res);
  console.log('ok', a, '->', b, res.length);
}
