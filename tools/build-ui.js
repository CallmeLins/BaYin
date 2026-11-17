#!/usr/bin/env node
/**
 * BaYin Car — 老 Android WebView 兼容构建
 *
 * 上游 UI 产物是 Tailwind v4（要求 Chrome 111+），而 Android 7.0~9 车机的
 * WebView 常年停在 Chrome 51~77。本脚本把官方产物转成可在老 WebView 上运行的版本：
 *
 *   1. CSS：按 chrome72 目标重写颜色（oklch/color-mix/lab → rgb/rgba）、
 *      展开 @layer、去掉 @property、把 dvh 降成 vh；
 *   2. JS ：按 chrome72/es2017 目标转译（解 ?. ?? 等），polyfill 交运行时；
 *   3. 注入：car.css（车机缩放/字号/边距/侧栏/标签栏）+ car.js（运行时自检 + 原生桥）。
 *
 * 产出：src-ui-gen/（tauri.conf.json 的 frontendDist 指向它）
 */
const fs = require('fs');
const path = require('path');

const ROOT = path.resolve(__dirname, '..');
const SRC = path.join(ROOT, 'src-ui');
const OUT = path.join(ROOT, 'src-ui-gen');
const CAR = path.join(SRC, 'car');

function log(...a) { console.log('[build-ui]', ...a); }
function fail(msg) { console.error('[build-ui] ERROR:', msg); process.exit(1); }

async function loadEsbuild() {
  const p = path.join(__dirname, 'node_modules', 'esbuild');
  if (!fs.existsSync(p)) fail('esbuild 未安装，请先运行：cd tools && npm install');
  return require(p);
}

function rmrf(dir) {
  if (fs.existsSync(dir)) fs.rmSync(dir, { recursive: true, force: true });
}

function ensure(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

async function main() {
  const esbuild = await loadEsbuild();
  if (!fs.existsSync(SRC)) fail(`未找到前端产物目录：${SRC}`);

  rmrf(OUT);
  ensure(path.join(OUT, 'assets'));

  const files = fs.readdirSync(path.join(SRC, 'assets'));
  let cssCount = 0, jsCount = 0;
  const renamed = new Map();

  for (const name of files) {
    const from = path.join(SRC, 'assets', name);
    if (!fs.statSync(from).isFile()) continue;

    if (name.endsWith('.css')) {
      const src = fs.readFileSync(from, 'utf8');
      // 1) lightningcss 做"值"级降级（oklch/lab/color() -> rgb）
      const lcss = require(path.join(__dirname, 'node_modules', 'lightningcss'));
      const lres = lcss.transform({
        filename: name,
        code: Buffer.from(src, 'utf8'),
        minify: true,
        targets: { chrome: 72 << 16 },
        errorRecovery: true,
      });
      let code = lres.code.toString();
      // 2) 自写后处理做"语法"级降级（@layer/@property/dvh）
      code = require('./css-legacy').postprocess(code);
      fs.writeFileSync(path.join(OUT, 'assets', name), code);
      cssCount++;
      log(`css  ${name}  ${src.length} -> ${code.length}`);
    } else if (name.endsWith('.js')) {
      const src = fs.readFileSync(from, 'utf8');
      const res = await esbuild.transform(src, {
        loader: 'js',
        target: ['chrome72', 'es2017'],
        minify: false,          // 已是产物，保持可读便于排错
        legalComments: 'none',
        charset: 'utf8',
      });
      if (res.warnings.length) res.warnings.slice(0, 5).forEach(w => log('js warn:', w.text));
      fs.writeFileSync(path.join(OUT, 'assets', name), res.code);
      jsCount++;
      log(`js   ${name}  ${src.length} -> ${res.code.length}`);
    } else {
      fs.copyFileSync(from, path.join(OUT, 'assets', name));
    }
  }

  // index.html：补 car.css / polyfill / car.js
  const htmlPath = path.join(SRC, 'index.html');
  let html = fs.readFileSync(htmlPath, 'utf8');
  html = html.replace(
    '<head>',
    '<head>\n    <meta name="color-scheme" content="dark light">'
  );
  // car.css 必须在上游样式之后
  html = html.replace(
    '</head>',
    '  <link rel="stylesheet" href="/car/car.css">\n  </head>'
  );
  // polyfill 必须在 bundle 之前同步执行
  html = html.replace(
    '<script type="module"',
    '<script src="/car/polyfill.js"></script>\n    <script type="module"'
  );
  // 面板样式 + 面板脚本（依赖 car.js 暴露的 apply）
  html = html.replace(
    '</head>',
    '  <link rel="stylesheet" href="/car/panel.css">\n  </head>'
  );
  html = html.replace(
    '</body>',
    '  <script src="/car/car.js"></script>\n  <script src="/car/panel.js"></script>\n  </body>'
  );
  fs.writeFileSync(path.join(OUT, 'index.html'), html);

  // 注入层
  const carOut = path.join(OUT, 'car');
  ensure(carOut);
  for (const f of fs.readdirSync(CAR)) {
    const from = path.join(CAR, f);
    if (!fs.statSync(from).isFile()) continue;
    if (f.endsWith('.js')) {
      const res = await esbuild.transform(fs.readFileSync(from, 'utf8'), {
        loader: 'js', target: ['chrome72', 'es2017'], charset: 'utf8',
      });
      fs.writeFileSync(path.join(carOut, f), res.code);
    } else {
      fs.copyFileSync(from, path.join(carOut, f));
    }
  }

  log(`完成：css=${cssCount} js=${jsCount} -> ${OUT}`);
  if (cssCount === 0) fail('没有处理到任何 CSS');
}

main().catch(e => fail(e && e.stack || String(e)));
