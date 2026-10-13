// Archive-wide host-boundary census: every identifier-boundary occurrence (not a
// property name after '.', not inside comments/strings/templates/regex) of the watch
// list, mapped to its enclosing top-level statement; plus every non-relative import.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const REPO = process.cwd();
const require = createRequire(REPO + '/packages/cli/');
const acorn = require('acorn');
const WATCH = new Set(['document','window','navigator','globalThis','addEventListener','removeEventListener','queueMicrotask','setTimeout','setInterval','clearTimeout','clearInterval','requestAnimationFrame','requestIdleCallback','MessageChannel','process','performance','localStorage','sessionStorage','fetch','Promise','Date','self','console','WeakRef','FinalizationRegistry','XMLHttpRequest','WebSocket','Worker','BroadcastChannel','structuredClone','postMessage','setImmediate','sharedConfig','_$HY','eval','Function','reportError','dispatchEvent','CustomEvent','EventTarget','serialize','require','importScripts','location','history','crypto','Headers','Response','Request','URL','ReadableStream','TextEncoder','MutationObserver','AbortController','Buffer','AsyncLocalStorage','Node','import']);
const [,, version, pkg, ...rels] = process.argv;
const base = version === 'rc9' ? REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/' : REPO + '/rust/target/audit-rc13/node_modules/';
const root = base + pkg;
function walk(dir){const o=[];for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isDirectory())o.push(...walk(p));else if(p.endsWith('.js'))o.push(p);}return o.sort();}
const files = rels.length ? rels.map(r => path.join(root, r)) : walk(path.join(root, 'dist'));
const result = {};
for (const f of files) {
  const src = fs.readFileSync(f, 'utf8');
  const ast = acorn.parse(src, { ecmaVersion: 'latest', sourceType: 'module', locations: true });
  const tops = ast.body.map(st => { let name = '<stmt>'; if (st.type === 'FunctionDeclaration' || st.type === 'ClassDeclaration') name = st.id?.name; else if (st.type === 'VariableDeclaration') name = st.declarations.map(d => d.id.name || '<pat>').join(','); else if (st.type.startsWith('Import')) name = '<import>'; else if (st.type.startsWith('Export')) name = '<export>'; return { s: st.start, e: st.end, name }; });
  const enclosing = (pos) => (tops.find(t => t.s <= pos && pos < t.e) || { name: '<top>' }).name;
  const rel = path.relative(root, f);
  const toks = [...acorn.tokenizer(src, { ecmaVersion: 'latest', sourceType: 'module', locations: true })];
  for (let i = 0; i < toks.length; i++) {
    const t = toks[i];
    let name = null;
    if (t.type.label === 'name' && WATCH.has(t.value)) name = t.value;
    if (t.type.keyword === 'import' && toks[i+1] && toks[i+1].type.label === '(') name = 'import()';
    if (t.type.keyword === 'import' && toks[i+1] && toks[i+1].type.label === '.') name = 'import.meta';
    if (!name) continue;
    const prev = toks[i - 1];
    if (prev && (prev.type.label === '.' || prev.type.label === '?.')) continue; // property name
    const next = toks[i + 1];
    if (next && next.type.label === ':' && prev && (prev.type.label === '{' || prev.type.label === ',')) continue; // object key (approx)
    const key = `${rel}|${name}|${enclosing(t.start)}`;
    (result[key] ??= []).push(t.loc.start.line);
  }
  for (const st of ast.body) if ((st.type === 'ImportDeclaration' || (st.type.startsWith('Export') && st.source)) && !st.source.value.startsWith('.')) { const key = `${rel}|IMPORT ${st.source.value}|<import>`; (result[key] ??= []).push(st.loc.start.line); }
}
for (const [k, v] of Object.entries(result)) console.log(k + '|' + v.join(','));
