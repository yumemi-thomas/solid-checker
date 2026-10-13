// Over-approximating, name-based call-graph aid (the rc.9 reads audit's callgraph.mjs,
// extended): per build, reach from roots; report read-entry calls, value uses, host
// identifiers, and compare each reached definition's text against the other version.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
const REPO = process.cwd();
const require = createRequire(REPO + '/packages/cli/');
const acorn = require('acorn');
const ROOTS = { rc9: REPO + '/rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/@solidjs/signals', rc13: REPO + '/rust/target/audit-rc13/node_modules/@solidjs/signals' };
function walkFiles(dir){const out=[];for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isDirectory())out.push(...walkFiles(p));else if(p.endsWith('.js'))out.push(p);}return out.sort();}
const builds={
  prod:(root)=>walkFiles(path.join(root,'dist/prod')).filter(f=>!/attribution/.test(f)),
  observe:(root)=>walkFiles(path.join(root,'dist/observe')).filter(f=>!/attribution\.js$|attribution-(costs|feedback|queries)/.test(f)),
  dev:(root)=>[path.join(root,'dist/dev.js'),path.join(root,'dist/dev-shared.js')],
};
function visit(node,cb,parent){ if(!node||typeof node.type!=='string')return; cb(node,parent); for(const k of Object.keys(node)){ if(k==='type'||k==='start'||k==='end')continue; const v=node[k]; if(Array.isArray(v))v.forEach(c=>visit(c,cb,node)); else if(v&&typeof v.type==='string')visit(v,cb,node);} }
export function load(version, buildName){
  const root=ROOTS[version]; const files=builds[buildName](root);
  const funcs=new Map();
  const add=(name,file,node,src)=>{ if(!funcs.has(name))funcs.set(name,[]); funcs.get(name).push({file:path.relative(root,file),node,text:src.slice(node.start,node.end)}); };
  for(const f of files){
    const src=fs.readFileSync(f,'utf8');
    const ast=acorn.parse(src,{ecmaVersion:'latest',sourceType:'module'});
    visit(ast,(n)=>{
      if(n.type==='FunctionDeclaration'&&n.id) add(n.id.name,f,n,src);
      if(n.type==='MethodDefinition'&&n.key&&(n.key.name||n.key.value)) add(n.key.name||String(n.key.value),f,n.value,src);
      if(process.env.EXT&&n.type==='Property'&&n.key&&n.key.type==='Identifier'&&n.value&&(n.value.type==='FunctionExpression'||n.value.type==='ArrowFunctionExpression')) add(n.key.name,f,n.value,src);
      if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'&&n.init&&(n.init.type==='ArrowFunctionExpression'||n.init.type==='FunctionExpression')) add(n.id.name,f,n.init,src);
      if(process.env.EXT&&n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&!n.left.computed&&n.left.property.type==='Identifier'&&(n.right.type==='Identifier')) { /* slot install: GlobalQueue.X = fn */ add('slot:'+n.left.property.name,f,n.right,src); }
    });
  }
  return funcs;
}
const READ=new Set((process.env.READ||'read,readNodeFast,serve,link,pendingCheckRead,latestRead,getLatestValueComputed').split(','));
const HOST=new Set(['document','window','navigator','globalThis','addEventListener','removeEventListener','queueMicrotask','setTimeout','setInterval','clearTimeout','clearInterval','requestAnimationFrame','requestIdleCallback','MessageChannel','process','performance','localStorage','sessionStorage','fetch','Promise','Date','self','console','WeakRef','FinalizationRegistry','XMLHttpRequest','WebSocket','Worker','BroadcastChannel','structuredClone','postMessage','setImmediate','sharedConfig','_$HY','eval','Function','reportError','dispatchEvent','CustomEvent','EventTarget','require','importScripts','location','history','crypto','serialize']);
function callsOf(node){
  const direct=new Set(), members=new Set(), valueUses=new Set(), host=new Set(), idents=new Set();
  visit(node,(n,p)=>{
    if(n.type==='CallExpression'||n.type==='NewExpression'){
      const c=n.callee;
      if(c.type==='Identifier')direct.add(c.name);
      else if(c.type==='MemberExpression'&&!c.computed&&c.property.type==='Identifier')members.add(c.property.name);
    }
    if(n.type==='Identifier'){
      const isProp = p && ((p.type==='MemberExpression'&&p.property===n&&!p.computed)||(p.type==='Property'&&p.key===n&&!p.computed&&!p.shorthand)||(p.type==='MethodDefinition'&&p.key===n));
      if(!isProp){ idents.add(n.name); if(HOST.has(n.name)) host.add(n.name); }
      if(READ.has(n.name)&&!isProp&&!(p&&p.type==='CallExpression'&&p.callee===n)&&!(p&&p.type==='FunctionDeclaration'))valueUses.add(n.name);
    }
  });
  return {direct,members,valueUses,host,idents};
}
export function reach(funcs, roots, {cut=new Set(), followIdents=false}={}){
  const seen=new Set(), queue=[...roots], readCalls=new Set(), valueUse=new Set(), host=new Map(), parent=new Map();
  roots.forEach(r=>parent.set(r,null));
  while(queue.length){
    const name=queue.shift(); if(seen.has(name))continue; seen.add(name); if(cut.has(name)&&!roots.includes(name))continue;
    for(const {node} of funcs.get(name)||[]){
      const {direct,members,valueUses,host:h,idents}=callsOf(node);
      const next=[...direct,...members,...(followIdents?idents:[])];
      for(const d of direct){ if(READ.has(d)) readCalls.add(name+'->'+d); }
      for(const m of members){ if(READ.has(m)) readCalls.add(name+'->.'+m); }
      for(const d of next){ if(funcs.has(d)&&!parent.has(d)){parent.set(d,name);} if(funcs.has(d)) queue.push(d); if(funcs.has('slot:'+d)) { const tgt=funcs.get('slot:'+d)[0].text; if(funcs.has(tgt)){ if(!parent.has(tgt)) parent.set(tgt,name+'[slot '+d+']'); queue.push(tgt);} } }
      valueUses.forEach(v=>valueUse.add(name+':'+v));
      h.forEach(x=>{ if(!host.has(x)) host.set(x,new Set()); host.get(x).add(name); });
    }
  }
  return {seen,readCalls,valueUse,host,parent};
}
export function pathTo(parent,t){ if(!parent.has(t)) return null; const c=[]; let cur=t; while(cur){ c.unshift(cur); cur=parent.get(cur); if(cur&&cur.includes('[slot')){ c.unshift(cur); cur=cur.split('[')[0]; } } return c.join(' -> '); }
export const textSha=(funcs,name)=>(funcs.get(name)||[]).map(x=>crypto.createHash('sha256').update(x.text).digest('hex').slice(0,16)).join('+');
if (import.meta.url === `file://${process.argv[1]}`) {
  const [,, buildName, ...roots] = process.argv;
  const cut=new Set((process.env.CUT||'').split(',').filter(Boolean));
  const f9=load('rc9',buildName), f13=load('rc13',buildName);
  const r9=reach(f9,roots,{cut}), r13=reach(f13,roots,{cut});
  console.log(`== ${buildName} ${roots.join(',')}: rc9 ${r9.seen.size} fns, rc13 ${r13.seen.size} fns`);
  console.log('  rc13 read-entry calls:',[...r13.readCalls].join(', ')||'none');
  console.log('  rc9  read-entry calls:',[...r9.readCalls].join(', ')||'none');
  console.log('  rc13 value uses:',[...r13.valueUse].join(', ')||'none');
  console.log('  rc13 host:',[...r13.host].map(([k,v])=>k+'{'+[...v].join(',')+'}').join(' '));
  const added=[...r13.seen].filter(n=>!r9.seen.has(n)), removed=[...r9.seen].filter(n=>!r13.seen.has(n));
  const changed=[...r13.seen].filter(n=>r9.seen.has(n)&&textSha(f9,n)!==textSha(f13,n));
  const same=[...r13.seen].filter(n=>r9.seen.has(n)&&textSha(f9,n)===textSha(f13,n));
  console.log('  newly reached in rc13:',added.join(' '));
  console.log('  no longer reached:',removed.join(' '));
  console.log('  reached, changed text:',changed.join(' '));
  console.log('  reached, identical text:',same.length);
  if(process.env.VERBOSE) console.log('  identical:',same.join(' '));
  if(process.env.WHY) for(const t of process.env.WHY.split(',')) console.log('  path13 '+t+': '+pathTo(r13.parent,t));
}
