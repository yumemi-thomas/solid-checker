import assert from 'node:assert/strict';
import {test} from 'node:test';
import {join,resolve} from 'node:path';
import {read} from './catalog.mjs';
import {transformAsyncReads,readProgram} from './async-read-transform-v1.mjs';
import {asyncReadFeedback} from './async-read-feedback-v1.mjs';
import {collectAsyncReadTrace,observedRead} from './async-read-runtime-v1.mjs';
const project=read('rust/target/cross-package-roots/run.json').results.find(row=>row.package==='rxjs').retainedArtifacts.projectDir;
const path=join(project,'src/__async_read_test.tsx');
const prelude=`import {createSignal,createMemo,untrack as ignore} from 'solid-js';`;
function transform(body,imports=prelude){return transformAsyncReads(imports+`function App(){const [value]=createSignal(1);${body}}`,path);}
for(const [name,body]of [
  ['direct returned callback',`return createMemo(()=>Promise.resolve().then(()=>value()));`],
  ['returned comparison',`return createMemo(()=>Promise.resolve().then(()=>value()>0));`],
  ['transparent TypeScript wrapper',`return createMemo(()=>Promise.resolve().then(()=>((value() as number) satisfies number)));`],
  ['async callback return',`return createMemo(()=>Promise.resolve().then(async()=>{await Promise.resolve();return value();}));`],
])test(name,()=>{const r=transform(body);assert.equal(r.sites.length,1);assert(r.code.includes('observedRead'));assert(r.map.sourcesContent[0].includes(body));});
for(const [name,body]of [
  ['synchronous compute read',`return createMemo(()=>value());`],
  ['discarded read',`return createMemo(()=>Promise.resolve().then(()=>{value();return 9;}));`],
  ['unknown argument use',`const log=(n:number)=>9;return createMemo(()=>Promise.resolve().then(()=>log(value())));`],
  ['exact untrack alias',`return createMemo(()=>Promise.resolve().then(()=>ignore(()=>value())));`],
  ['not inside a memo',`return Promise.resolve().then(()=>value());`],
])test(name+' remains uninstrumented',()=>assert.equal(transform(body).sites.length,0));
test('namespace imports use exact core symbols',()=>{const r=transformAsyncReads(`import * as S from 'solid-js';function App(){const [v]=S.createSignal(1);return S.createMemo(()=>Promise.resolve().then(()=>v()));}`,path);assert.equal(r.sites.length,1);});
test('a shadowed creator is not a native accessor',()=>{const r=transformAsyncReads(`import {createMemo} from 'solid-js';function App(){const createSignal=(n:number)=>[()=>n] as const;const [v]=createSignal(1);return createMemo(()=>Promise.resolve().then(()=>v()));}`,path);assert.equal(r.sites.length,0);});
test('actual typing errors suppress all instrumentation',()=>{const r=transform(`value=()=>2;return createMemo(()=>Promise.resolve().then(()=>value()));`);assert.equal(r.sites.length,0);assert(r.open[0].codes.includes(2588));});
test('runtime helper returns identity and preserves observer/owner state',()=>{const old=globalThis.__asyncReadTrace,trace=collectAsyncReadTrace(),value={n:1},site={path:'consumer',sourceSha256:'digest',start:1};
  globalThis.__asyncReadTrace=trace;let observer=null,owner=null;
  try{assert.equal(observedRead(value,JSON.stringify(site),()=>observer,()=>owner),value);observedRead(value,site,()=>observer,()=>owner);
    assert.equal(trace.events.length,1);assert.equal(trace.events[0].occurrences,2);observer={};owner={};observedRead(value,site,()=>observer,()=>owner);
    assert.equal(trace.events.length,2);assert.deepEqual(trace.events[1].context,{observer:true,owner:true});
  }finally{globalThis.__asyncReadTrace=old;}
});
test('feedback requires exact current source and absent context',()=>{const code=prelude+`function App(){const [v]=createSignal(1);return createMemo(()=>Promise.resolve().then(()=>v()));}`;
  const transformed=transformAsyncReads(code,path),program=readProgram(code,path),source=program.getSourceFile(path),site=transformed.sites[0];
  const event={site,context:{observer:false,owner:false},occurrences:1};
  const result=asyncReadFeedback(program,source,[event]);assert.equal(result.notes.length,1);assert.equal(result.notes[0].severity,'info');assert.equal(result.notes[0].certification,false);
  assert.equal(asyncReadFeedback(program,source,[{...event,context:{observer:true,owner:true}}]).notes.length,0);
  const changed=readProgram(code+'\n// changed',path);assert.equal(asyncReadFeedback(changed,changed.getSourceFile(path),[event]).notes.length,0);
  assert.equal(asyncReadFeedback(program,source,[{...event,site:{...site,start:site.start+1}}]).notes.length,0);
});
