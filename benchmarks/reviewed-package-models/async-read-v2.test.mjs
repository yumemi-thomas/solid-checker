import assert from 'node:assert/strict';
import {test} from 'node:test';
import {join} from 'node:path';
import {read} from './catalog.mjs';
import {transformAsyncReadsV2,readProgram} from './async-read-transform-v2.mjs';
import {asyncReadFeedbackV2} from './async-read-feedback-v2.mjs';
const root=read('rust/target/cross-package-roots/run.json').results.find(row=>row.package==='neverthrow').retainedArtifacts.projectDir;
const path=join(root,'src/__async_read_v2_test.tsx'),imports=`import {createSignal,createMemo,untrack} from 'solid-js';import * as S from 'solid-js';`;
function run(body){return transformAsyncReadsV2(imports+`function App(){const [value]=createSignal(1);${body}}`,path);}
for(const [name,expression]of [ ['object','({n:value()})'],['array','[value()]'],['nested result','({a:[{n:value()}]})'] ])
  test(name+' returned read has a current witness',()=>{const result=run(`return createMemo(()=>Promise.resolve().then(()=>${expression}));`);assert.equal(result.sites.length,1);});
test('immutable getter alias chain preserves exact native creation',()=>{const result=run(`const a=value,b=a;return createMemo(()=>Promise.resolve().then(()=>b()));`);
  assert.equal(result.sites.length,1);assert.equal(result.sites[0].accessorAliases.length,2);});
test('local immutable native function aliases are resolved',()=>{const result=run(`const derive=createMemo;return derive(()=>Promise.resolve().then(()=>value()));`);assert.equal(result.sites.length,1);});
test('local immutable untrack aliases express intent',()=>{const result=run(`const ignore=S.untrack;return createMemo(()=>Promise.resolve().then(()=>ignore(()=>value())));`);assert.equal(result.sites.length,0);});
for(const [name,body]of [
  ['unused result',`return createMemo(()=>Promise.resolve().then(()=>{const inspected=(()=>value())();return 9;}));`],
  ['discarded call',`return createMemo(()=>Promise.resolve().then(()=>{(()=>value())();return 9;}));`],
  ['spread array',`return createMemo(()=>Promise.resolve().then(()=>[value(),...[9]]));`],
  ['spread object',`return createMemo(()=>Promise.resolve().then(()=>({n:value(),...{other:9}})));`],
  ['mutable accessor alias',`let alias=value;return createMemo(()=>Promise.resolve().then(()=>alias()));`],
  ['deferred object getter',`return createMemo(()=>Promise.resolve().then(()=>({get n(){return value();}})));`],
])test(name+' stays open',()=>assert.equal(run(body).sites.length,0));
test('a retained returned object hint refuses changed source bytes',()=>{const code=imports+`function App(){const [v]=createSignal(1);return createMemo(()=>Promise.resolve().then(()=>({n:v()})));}`;
  const site=transformAsyncReadsV2(code,path).sites[0],program=readProgram(code,path),source=program.getSourceFile(path),event={site,context:{owner:false,observer:false},occurrences:1};
  assert.equal(asyncReadFeedbackV2(program,source,[event]).notes.length,1);const changed=readProgram(code+'\n// changed',path);
  assert.equal(asyncReadFeedbackV2(changed,changed.getSourceFile(path),[event]).notes.length,0);
});
