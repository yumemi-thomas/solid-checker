// For named dev definitions: callee names called in rc13 but not in rc9.
import { load } from './closure.mjs';
import { createRequire } from 'node:module';
const require = createRequire(process.cwd() + '/packages/cli/');
function visit(node,cb){ if(!node||typeof node.type!=='string')return; cb(node); for(const k of Object.keys(node)){ if(k==='type'||k==='start'||k==='end')continue; const v=node[k]; if(Array.isArray(v))v.forEach(c=>visit(c,cb)); else if(v&&typeof v.type==='string')visit(v,cb);} }
const [,, build, ...names] = process.argv;
const a = load('rc9', build), b = load('rc13', build);
const calls = (f, n) => { const s = new Set(); for (const { node } of f.get(n) || []) visit(node, x => { if (x.type === 'CallExpression' || x.type === 'NewExpression') { const c = x.callee; s.add(c.type === 'Identifier' ? c.name : c.type === 'MemberExpression' ? (c.object.type === 'Identifier' ? c.object.name : (c.object.type==='MemberExpression' && !c.object.computed ? '*.'+c.object.property.name : '*')) + '.' + (c.computed ? '[]' : c.property.name) : '<' + c.type + '>'); } }); return s; };
for (const n of names) { const x = calls(a, n), y = calls(b, n); console.log(n, ' +', [...y].filter(c => !x.has(c)).join(' '), ' -', [...x].filter(c => !y.has(c)).join(' ')); }
