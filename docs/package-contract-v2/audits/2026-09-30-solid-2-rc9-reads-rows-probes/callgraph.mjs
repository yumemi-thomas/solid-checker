// Call-graph aid for the reads audit (over-approximating, name-based).
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
const WT = '/Users/thomas/Documents/Github/solid-checker/.claude/worktrees/agent-ade6c6119d231af5a';
const require = createRequire(WT + '/packages/cli/');
const acorn = require('acorn');
const [,, root, buildName, ...roots] = process.argv;   // root = package dir
function walkFiles(dir){const out=[];for(const e of fs.readdirSync(dir,{withFileTypes:true})){const p=path.join(dir,e.name);if(e.isDirectory())out.push(...walkFiles(p));else if(p.endsWith('.js'))out.push(p);}return out;}
const builds={
  prod:()=>walkFiles(path.join(root,'dist/prod')),
  observe:()=>walkFiles(path.join(root,'dist/observe')),
  dev:()=>[path.join(root,'dist/dev.js'),path.join(root,'dist/dev-shared.js')],
};
const files=builds[buildName]();
const funcs=new Map(); // name -> [{file,node}]
function add(name,file,node){ if(!funcs.has(name))funcs.set(name,[]); funcs.get(name).push({file,node}); }
function visit(node,cb,parent){ if(!node||typeof node.type!=='string')return; cb(node,parent); for(const k of Object.keys(node)){ if(k==='type'||k==='start'||k==='end')continue; const v=node[k]; if(Array.isArray(v))v.forEach(c=>visit(c,cb,node)); else if(v&&typeof v.type==='string')visit(v,cb,node);} }
for(const f of files){
  const src=fs.readFileSync(f,'utf8');
  const ast=acorn.parse(src,{ecmaVersion:'latest',sourceType:'module'});
  visit(ast,(n,p)=>{
    if(n.type==='FunctionDeclaration'&&n.id) add(n.id.name,f,n);
    if(n.type==='MethodDefinition'&&n.key&&n.key.name) add(n.key.name,f,n.value);
    if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'&&n.init&&(n.init.type==='ArrowFunctionExpression'||n.init.type==='FunctionExpression')) add(n.id.name,f,n.init);
  });
}
const CUT=new Set((process.env.CUT||'').split(',').filter(Boolean));
const READ=new Set(['read','readNodeFast','serve','link','pendingCheckRead','latestRead','getLatestValueComputed']);
function callsOf(node){
  const direct=new Set(), members=new Set(), valueUses=new Set();
  visit(node,(n,p)=>{
    if(n.type==='CallExpression'||n.type==='NewExpression'){
      const c=n.callee;
      if(c.type==='Identifier')direct.add(c.name);
      else if(c.type==='MemberExpression'&&!c.computed&&c.property.type==='Identifier')members.add(c.property.name);
      else if(c.type==='MemberExpression')members.add('<computed>');
      else members.add('<'+c.type+'>');
    }
    if(n.type==='Identifier'&&READ.has(n.name)&&!(p&&p.type==='CallExpression'&&p.callee===n)&&!(p&&p.type==='FunctionDeclaration')&&!(p&&p.type==='MemberExpression'&&p.property===n&&!p.computed))valueUses.add(n.name);
  });
  return {direct,members,valueUses};
}
for(const r of roots){
  const seen=new Set(), queue=[r], readCalls=new Set(), valueUse=new Set(), unresolvedMembers=new Set();
  const edges=[];
  while(queue.length){
    const name=queue.pop(); if(seen.has(name))continue; seen.add(name); if(CUT.has(name)&&name!==r)continue;
    for(const {node} of funcs.get(name)||[]){
      const {direct,members,valueUses}=callsOf(node);
      for(const d of direct){ if(READ.has(d)) readCalls.add(name+'->'+d); if(funcs.has(d)) queue.push(d); }
      for(const m of members){ if(READ.has(m)) readCalls.add(name+'->.'+m); if(funcs.has(m)) queue.push(m); else unresolvedMembers.add(m); }
      valueUses.forEach(v=>valueUse.add(name+':'+v));
    }
  }
  console.log(`== ${buildName} ${r}: ${seen.size} functions`);
  console.log('  read-entry calls:',[...readCalls].join(', ')||'none');
  console.log('  value uses of read entries:',[...valueUse].join(', ')||'none');
  if(process.env.VERBOSE) console.log('  functions:',[...seen].sort().join(' '));
  if(process.env.VERBOSE) console.log('  unresolved member calls:',[...unresolvedMembers].sort().join(' '));
}
if(process.env.WHY){
  for(const target of process.env.WHY.split(',')){
    const seen=new Map(); const queue=[[roots[0],null]];
    // BFS with parents
    const parent=new Map(); const q=[roots[0]]; parent.set(roots[0],null);
    while(q.length){ const name=q.shift(); if(CUT.has(name)&&name!==roots[0])continue;
      for(const {node} of funcs.get(name)||[]){ const {direct,members}=callsOf(node);
        for(const d of [...direct,...members]){ if(funcs.has(d)&&!parent.has(d)){parent.set(d,name);q.push(d);} } } }
    let chain=[],cur=target; if(!parent.has(cur)){console.log('  '+target+': unreachable');continue;}
    while(cur){chain.unshift(cur);cur=parent.get(cur);} console.log('  path to '+target+': '+chain.join(' -> '));
  }
}
