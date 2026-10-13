import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,symlinkSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {test} from 'node:test';
import {read} from './catalog.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformProjectReads} from './async-read-transform-v3.mjs';

const installed=read('rust/target/cross-package-roots/run.json').results.find(row=>row.package==='neverthrow').retainedArtifacts.projectDir;
const code=`import {createSignal,createMemo} from 'solid-js';import {initial} from '@/initial';
export function App(){const n:number=environment;const [value]=createSignal(initial+n);return createMemo(()=>Promise.resolve().then(()=>value()));}`;
function fixture(run){const root=mkdtempSync(join(tmpdir(),'solid-read-project-'));mkdirSync(join(root,'src'));symlinkSync(join(installed,'node_modules'),join(root,'node_modules'),'dir');
  writeFileSync(join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:true,skipLibCheck:true,paths:{'@/*':['./src/*']}},include:['src']}));
  const path=join(root,'src/main.tsx');writeFileSync(path,code);writeFileSync(join(root,'src/initial.ts'),'export const initial=1;');writeFileSync(join(root,'src/env.d.ts'),'declare const environment:number;');
  try{return run({root,path,session:projectReadSession(root)});}finally{rmSync(root,{recursive:true,force:true});}}
test('real aliases and ambient declarations admit an exact native read',()=>fixture(({path,session})=>{const result=transformProjectReads(code,path,session);assert.equal(result.sites.length,1);assert.equal(result.open.length,0);}));
test('unchanged project inputs reuse one program',()=>fixture(({path,session})=>{const first=session.get(path,code),second=session.get(path,code);assert.equal(first.program,second.program);assert.equal(second.reused,true);assert.equal(session.stats.builds,1);}));
test('served code from another source revision is refused',()=>fixture(({path,session})=>{assert.throws(()=>session.get(path,code+'\n'),/Consumer bytes differ/);}));
test('a changed alias target invalidates the program and respects real types',()=>fixture(({root,path,session})=>{const first=session.get(path,code);writeFileSync(join(root,'src/initial.ts'),`export const initial={};`);const second=session.get(path,code);assert.notEqual(first.program,second.program);assert(second.errors.some(error=>error.code===2365));assert.equal(transformProjectReads(code,path,session).sites.length,0);}));
test('a changed ambient declaration invalidates the program',()=>fixture(({root,path,session})=>{const first=session.get(path,code);writeFileSync(join(root,'src/env.d.ts'),'declare const environment:string;');const second=session.get(path,code);assert.notEqual(first.program,second.program);assert(second.errors.some(error=>error.code===2322));}));
test('new included ambient files invalidate a previously clean project',()=>fixture(({root,path,session})=>{session.get(path,code);writeFileSync(join(root,'src/new.ts'),'const broken:number="x";');const next=session.get(path,code);assert.equal(next.generation,2);assert(next.errors.some(error=>error.code===2322));}));
test('a previously missing import invalidates when its file appears',()=>fixture(({root,path,session})=>{const missing=code.replace("'@/initial'","'@/later'");writeFileSync(path,missing);const first=session.get(path,missing);assert(first.errors.some(error=>error.code===2307));writeFileSync(join(root,'src/later.ts'),'export const initial=2;');const second=session.get(path,missing);assert.equal(second.generation,2);assert.equal(second.errors.length,0);}));
test('configuration changes invalidate exact source selection',()=>fixture(({root,path,session})=>{session.get(path,code);writeFileSync(join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{jsx:'preserve',jsxImportSource:'@solidjs/web',skipLibCheck:true},files:['src/initial.ts']}));assert.throws(()=>session.get(path,code),/absent from the configured/);}));
test('an original consumer edit causes a new source epoch',()=>fixture(({path,session})=>{const first=transformProjectReads(code,path,session),changed=code+'\n// current source';writeFileSync(path,changed);const next=transformProjectReads(changed,path,session);assert.equal(next.session.generation,2);assert.notEqual(first.sites[0].sourceSha256,next.sites[0].sourceSha256);}));
