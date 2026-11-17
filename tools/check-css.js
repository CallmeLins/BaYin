const fs = require('fs');
const css = fs.readFileSync('D:/AppData/DSH_Work/BaYin_Car-main/src-ui-gen/assets/index-B443b0Rt.css', 'utf8');
function matchBrace(s, start) {
  let d = 0;
  for (let i = start; i < s.length; i++) {
    if (s[i] === '{') d++;
    else if (s[i] === '}') { d--; if (d === 0) return i + 1; }
  }
  return -1;
}
function stripAtSupports(s) {
  let out = '', i = 0;
  while (i < s.length) {
    const j = s.indexOf('@supports', i);
    if (j < 0) { out += s.slice(i); break; }
    out += s.slice(i, j);
    const k = s.indexOf('{', j);
    if (k < 0) { out += s.slice(j); break; }
    i = matchBrace(s, k);
  }
  return out;
}
const outside = stripAtSupports(css);
console.log('total color-mix      :', (css.match(/color-mix\(/g) || []).length);
console.log('outside @supports    :', (outside.match(/color-mix\(/g) || []).length);
console.log('@supports blocks     :', (css.match(/@supports/g) || []).length);
const bad = outside.match(/color-mix\([^)]*\)/g) || [];
console.log('ungated samples:', bad.slice(0, 5));
