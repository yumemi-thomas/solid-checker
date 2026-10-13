import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
import {mayHaveNativeCallbackRead} from './async-read-prefilter-v1.mjs';
import {transformCandidateProjectReads} from './async-read-transform-v4.mjs';

const head=`import {createSignal,createMemo} from 'solid-js';`;
for(const [label,body]of [
 ['direct async callback',`const [v]=createSignal(1);createMemo(()=>Promise.resolve().then(()=>v()));`],
 ['named reader',`const [v]=createSignal(1);createMemo(()=>{function read(){return v();}return Promise.resolve().then(read);});`],
 ['transparent read wrapper',`const [v]=createSignal(1);createMemo(()=>Promise.resolve().then(()=>(v as ()=>number)()));`],
 ['immutable memo alias',`const [v]=createSignal(1),derive=createMemo;derive(()=>Promise.resolve().then(()=>v()));`],
 ['namespace imports',`import * as S from 'solid-js';const [v]=S.createSignal(1);S.createMemo(()=>Promise.resolve().then(()=>v()));`],
])test(label+' passes the necessary syntax gate',()=>assert.equal(mayHaveNativeCallbackRead(head+body,'app.tsx'),true));
for(const [label,body]of [
 ['ordinary signal UI',`const [v]=createSignal(1);function App(){return <p>{v()}</p>;}`],
 ['direct memo read',`const [v]=createSignal(1);function App(){return createMemo(()=>v());}`],
 ['no Solid import',`const v=()=>1;memo(()=>Promise.resolve().then(()=>v()));`],
])test(label+' skips the expensive session',()=>{const code=label==='no Solid import'?body:head+body;
 assert.equal(mayHaveNativeCallbackRead(code,'app.tsx'),false);const result=transformCandidateProjectReads(code,'app.tsx',{get(){throw Error('program must not be requested');}});assert.equal(result.code,code);assert.equal(result.sites.length,0);});
test('the gate preserves every enrolled site in the retained real-project inventory',()=>{
 const inventory=JSON.parse(readFileSync('rust/target/async-read-real-app-inventory-v1.json','utf8'));
 let sites=0;for(const project of inventory.projects)for(const file of project.files??[])if(file.sites.length){assert(mayHaveNativeCallbackRead(readFileSync(file.path,'utf8'),file.path),file.path);sites+=file.sites.length;}
 assert.equal(sites,7);
});
