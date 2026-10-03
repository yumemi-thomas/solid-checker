import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,writeFileSync,symlinkSync,rmSync} from 'node:fs';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
import {test} from 'node:test';
import {read} from './catalog.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformProjectReads} from './async-read-transform-v3.mjs';
const installed=read('rust/target/cross-package-roots/run.json').results.find(row=>row.package==='neverthrow').retainedArtifacts.projectDir;
const code=`import {createSignal,createMemo} from 'solid-js';export function App(){const [value]=createSignal(1);return createMemo(()=>Promise.resolve().then(()=>value()));}`;
function fixture(run){const root=mkdtempSync(join(tmpdir(),'solid-project-boundary-'));mkdirSync(join(root,'src'));symlinkSync(join(installed,'node_modules'),join(root,'node_modules'),'dir');const path=join(root,'src/main.tsx');writeFileSync(path,code);
 try{return run({root,path,session:projectReadSession(root)});}finally{rmSync(root,{recursive:true,force:true});}}
test('unconfigured projects rebuild when a second original source enters',()=>fixture(({root,path,session})=>{
 assert.equal(transformProjectReads(code,path,session).sites.length,1);const second=join(root,'src/second.tsx');writeFileSync(second,code);
 const result=transformProjectReads(code,second,session);assert.equal(result.sites.length,1);assert.equal(result.session.generation,2);
}));
test('referenced TypeScript projects remain explicitly unsupported',()=>fixture(({root,path,session})=>{
 writeFileSync(join(root,'tsconfig.json'),JSON.stringify({files:['src/main.tsx'],references:[{path:'./library'}]}));
 assert.throws(()=>session.get(path,code),/Referenced TypeScript projects need an explicit project session/);
}));
test('a consumer outside the project has no cached program authority',()=>fixture(({path})=>{
 const session=projectReadSession(join(installed,'src'));assert.throws(()=>session.get(path,code),/outside the project session/);
}));
test('an inherited alias configuration participates in invalidation',()=>fixture(({root,path})=>{
 const options={target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:true,skipLibCheck:true,paths:{'@/*':['./src/*']}};
 writeFileSync(join(root,'base.json'),JSON.stringify({compilerOptions:options}));writeFileSync(join(root,'tsconfig.json'),JSON.stringify({extends:'./base.json',include:['src']}));
 writeFileSync(join(root,'src/initial.ts'),'export const initial=1;');const aliased=code.replace('export function App()',`import {initial} from '@/initial';export function App()`).replace('createSignal(1)','createSignal(initial)');writeFileSync(path,aliased);
 const session=projectReadSession(root);assert.equal(transformProjectReads(aliased,path,session).sites.length,1);
 writeFileSync(join(root,'base.json'),JSON.stringify({compilerOptions:{...options,paths:{'@/*':['./missing/*']}}}));const result=transformProjectReads(aliased,path,session);
 assert.equal(result.session.generation,2);assert.equal(result.sites.length,0);assert(result.open[0].codes.includes(2307));
}));
