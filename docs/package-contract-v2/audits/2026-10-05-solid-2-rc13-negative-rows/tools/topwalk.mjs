// Breadth-first walk over top-level definitions of ONE file (the rc.9 merge/omit and
// core-and-web audits' method): from roots, follow every identifier token that names
// another top-level definition of the same file; list reached definitions with their
// text digest, the imported bindings and the well-known globals they reference.
// usage: node topwalk.mjs <pkgdir> <relfile> root... ; env CMP=1 compares rc9 vs rc13
import fs from 'node:fs';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
const REPO = process.cwd();
const require = createRequire(REPO + '/packages/cli/');
const acorn = require('acorn');
const BASE = { rc9: REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/', rc13: REPO + '/rust/target/audit-rc13/node_modules/' };
const GLOBALS = new Set(['Map','Set','WeakMap','WeakSet','Object','Proxy','Reflect','Symbol','Array','Promise','Error','TypeError','JSON','Math','Number','String','Boolean','queueMicrotask','setTimeout','clearTimeout','setInterval','console','globalThis','window','document','fetch','performance','process','WeakRef','FinalizationRegistry','structuredClone','Date','URL','Headers','Response','Request','ReadableStream','TextEncoder','AbortController','import','eval','Function','reportError','navigator','location','history','self','MutationObserver','requestAnimationFrame','Node','Element','HTMLElement','DocumentFragment','Event','CustomEvent','AsyncLocalStorage','Buffer','crypto','undefined','NaN','Infinity','isNaN','parseInt','parseFloat','BigInt','Int32Array','Uint8Array','ArrayBuffer','encodeURIComponent','decodeURIComponent','escape','RegExp','AggregateError','DOMException']);
export function topDefs(version, pkg, rel) {
  const src = fs.readFileSync(BASE[version] + pkg + '/' + rel, 'utf8');
  const ast = acorn.parse(src, { ecmaVersion: 'latest', sourceType: 'module' });
  const defs = new Map(); const imports = new Map();
  const add = (n, node) => { const t = src.slice(node.start, node.end); const prev = defs.get(n); defs.set(n, prev ? { text: prev.text + '\n' + t, node } : { text: t, node }); };
  for (const st of ast.body) {
    if ((st.type === 'FunctionDeclaration' || st.type === 'ClassDeclaration') && st.id) add(st.id.name, st);
    else if (st.type === 'VariableDeclaration') for (const d of st.declarations) { if (d.id.type === 'Identifier') add(d.id.name, st); else if (d.id.type === 'ObjectPattern' || d.id.type === 'ArrayPattern') { const names = []; (function g(p){ if(!p) return; if(p.type==='Identifier') names.push(p.name); else if(p.type==='ObjectPattern') p.properties.forEach(q=>g(q.value||q.argument)); else if(p.type==='ArrayPattern') p.elements.forEach(g); else if(p.type==='AssignmentPattern') g(p.left); else if(p.type==='RestElement') g(p.argument);})(d.id); names.forEach(n=>add(n, st)); } }
    else if (st.type === 'ImportDeclaration') for (const s of st.specifiers) imports.set(s.local.name, (s.imported ? (s.imported.name || s.imported.value) : 'default') + ' from ' + st.source.value);
    else if (st.type === 'ExpressionStatement') { // top-level assignments like `x.y = ...` keep as anonymous
      (defs.get('<expr>') || defs.set('<expr>', { text: '', node: st }).get('<expr>')).text += src.slice(st.start, st.end) + '\n';
    }
  }
  return { defs, imports, src };
}
export function walkFrom(version, pkg, rel, roots, cut = new Set()) {
  const { defs, imports } = topDefs(version, pkg, rel);
  const seen = new Map(), usedImports = new Map(), globals = new Map(); const q = [...roots];
  while (q.length) {
    const n = q.shift(); if (seen.has(n) || !defs.has(n)) continue;
    const d = defs.get(n); seen.set(n, crypto.createHash('sha256').update(d.text).digest('hex').slice(0, 16));
    if (cut.has(n) && !roots.includes(n)) continue;
    const toks = [...acorn.tokenizer(d.text, { ecmaVersion: 'latest', sourceType: 'module' })];
    for (let i = 0; i < toks.length; i++) {
      const t = toks[i]; if (t.type.label !== 'name' && !(t.type.keyword === 'import')) continue;
      const prev = toks[i - 1]; if (prev && (prev.type.label === '.' || prev.type.label === '?.')) continue;
      const v = t.value;
      if (defs.has(v) && v !== n) q.push(v);
      if (imports.has(v)) (usedImports.get(v) || usedImports.set(v, new Set()).get(v)).add(n);
      if (GLOBALS.has(v) && !defs.has(v)) (globals.get(v) || globals.set(v, new Set()).get(v)).add(n);
    }
  }
  return { seen, usedImports: new Map([...usedImports].map(([k, v]) => [k + ' (' + imports.get(k) + ')', v])), globals };
}
if (import.meta.url === `file://${process.argv[1]}`) {
  const [,, pkg, rel, ...roots] = process.argv;
  const cut = new Set((process.env.CUT || '').split(',').filter(Boolean));
  const a = walkFrom('rc9', pkg, rel, roots, cut), b = walkFrom('rc13', pkg, rel, roots, cut);
  console.log(`== ${pkg}/${rel} from ${roots.join(',')}: rc9 ${a.seen.size} defs, rc13 ${b.seen.size} defs`);
  const names = [...new Set([...a.seen.keys(), ...b.seen.keys()])].sort();
  const st = (n) => !a.seen.has(n) ? 'NEW' : !b.seen.has(n) ? 'GONE' : a.seen.get(n) === b.seen.get(n) ? 'same' : 'CHANGED';
  for (const s of ['CHANGED', 'NEW', 'GONE', 'same']) console.log(`  ${s}: ${names.filter(n => st(n) === s).join(' ')}`);
  console.log('  rc13 imports used:', [...b.usedImports].map(([k, v]) => k + '<-' + [...v].join(',')).join('; '));
  console.log('  rc13 globals:', [...b.globals].map(([k, v]) => k + '<-' + [...v].join(',')).join('; '));
  if (process.env.SHOW9) console.log('  rc9 imports used:', [...a.usedImports].map(([k]) => k).join('; '), '\n  rc9 globals:', [...a.globals].map(([k]) => k).join(' '));
}
