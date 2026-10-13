// Hook-slot follow-up (dev build): for a root's closure, list `GlobalQueue._x(...)` /
// `optHooks.x(...)` style slot calls, resolve each slot to the function assigned to it
// anywhere in the build (`X._x = fn` or `X._x = (...) => ...`), and report whether the
// slot target's own closure reaches a read entry point.
import fs from 'node:fs';
import { load, reach, pathTo } from './closure.mjs';
import { createRequire } from 'node:module';
const require = createRequire(process.cwd() + '/packages/cli/');
const acorn = require('acorn');
const REPO = process.cwd();
function visit(node,cb,parent){ if(!node||typeof node.type!=='string')return; cb(node,parent); for(const k of Object.keys(node)){ if(k==='type'||k==='start'||k==='end')continue; const v=node[k]; if(Array.isArray(v))v.forEach(c=>visit(c,cb,node)); else if(v&&typeof v.type==='string')visit(v,cb,node);} }
const [,, version, build, ...roots] = process.argv;
const cut = new Set((process.env.CUT || '').split(',').filter(Boolean));
const funcs = load(version, build);
const base = version === 'rc9' ? REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/@solidjs/signals/dist/' : REPO + '/rust/target/audit-rc13/node_modules/@solidjs/signals/dist/';
const files = build === 'dev' ? ['dev.js', 'dev-shared.js'] : null;
// slot assignments: OBJ.name = <Identifier | function>
const assigns = new Map();
for (const f of files) {
  const src = fs.readFileSync(base + f, 'utf8');
  const ast = acorn.parse(src, { ecmaVersion: 'latest', sourceType: 'module' });
  visit(ast, (n) => {
    if (n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression' && !n.left.computed && n.left.object.type === 'Identifier' && /^(GlobalQueue|globalQueue|optHooks|attrHooks)$/.test(n.left.object.name)) {
      const k = n.left.object.name + '.' + n.left.property.name;
      const v = n.right.type === 'Identifier' ? n.right.name : '<inline ' + n.right.type + '>';
      (assigns.get(k) || assigns.set(k, new Set()).get(k)).add(v);
    }
    if (n.type === 'VariableDeclarator' && n.id.name === 'optHooks' && n.init && n.init.type === 'ObjectExpression') for (const p of n.init.properties) if (p.key) (assigns.get('optHooks.' + (p.key.name)) || assigns.set('optHooks.' + p.key.name, new Set()).get('optHooks.' + p.key.name)).add(p.value.type === 'Identifier' ? p.value.name : '<inline>');
  });
}
const r = reach(funcs, roots, { cut });
const slotCalls = new Map();
for (const name of r.seen) for (const { node } of funcs.get(name) || []) visit(node, (n) => {
  if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && !n.callee.computed && n.callee.object.type === 'Identifier' && /^(GlobalQueue|globalQueue|optHooks)$/.test(n.callee.object.name) && /^_|^[a-z]/.test(n.callee.property.name)) {
    const k = n.callee.object.name + '.' + n.callee.property.name;
    (slotCalls.get(k) || slotCalls.set(k, new Set()).get(k)).add(name);
  }
});
for (const [k, callers] of [...slotCalls].sort()) {
  const alt = k.replace(/^globalQueue/, 'GlobalQueue');
  const targets = [...(assigns.get(k) || []), ...(assigns.get(alt) || [])];
  const res = targets.map(t => { if (!funcs.has(t)) return t + '(?)'; const rr = reach(funcs, [t], { cut }); return `${t}[${rr.seen.size}fn reads:${[...rr.readCalls].join(',') || 'none'}]`; });
  console.log(`${k} <- ${[...callers].join(',')} => ${res.join(' ; ') || '(method or unassigned)'}`);
}
