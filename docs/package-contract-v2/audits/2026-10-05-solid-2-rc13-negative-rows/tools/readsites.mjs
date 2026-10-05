// Archive-wide read-site census: which top-level/method functions call a read entry
// point (or use one as a value), per build and version.
import { load } from './closure.mjs';
const READ = new Set((process.env.READ || 'read,readNodeFast,serve,link,pendingCheckRead,latestRead,getLatestValueComputed').split(','));
import { createRequire } from 'node:module';
const require = createRequire(process.cwd() + '/packages/cli/');
const acorn = require('acorn');
function visit(node,cb,parent){ if(!node||typeof node.type!=='string')return; cb(node,parent); for(const k of Object.keys(node)){ if(k==='type'||k==='start'||k==='end')continue; const v=node[k]; if(Array.isArray(v))v.forEach(c=>visit(c,cb,node)); else if(v&&typeof v.type==='string')visit(v,cb,node);} }
for (const build of ['prod', 'observe', 'dev']) for (const v of ['rc9', 'rc13']) {
  const funcs = load(v, build); const hits = new Set();
  for (const [name, defs] of funcs) for (const { node } of defs) visit(node, (n, p) => {
    if (n.type === 'Identifier' && READ.has(n.name)) {
      const isProp = p && p.type === 'MemberExpression' && p.property === n && !p.computed;
      const isDecl = p && p.type === 'FunctionDeclaration' && p.id === n;
      if (!isDecl && !(isProp && !(p && false))) hits.add(name);
      if (isProp && p.object && p.object.type === 'Identifier' && false) hits.add(name);
    }
  });
  console.log(build, v, [...hits].sort().join(' '));
}
