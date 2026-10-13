import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,symlinkSync,cpSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {createNativeReads} from './native-read-runtime-v1.mjs';
import {instrumentNativeReads,nativeReaderPremise} from './native-read-hook-v1.mjs';
import nativeReadPlugin,{transformNativeReads} from './async-read-transform-v6.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {mayHaveNativeCallbackRead} from './async-read-prefilter-v2.mjs';
const install=new URL('../../rust/target/app-import-metric/apps/helge-dev',import.meta.url).pathname,
  native=join(install,'node_modules/@solidjs/signals/dist'),site={path:'consumer',sourceSha256:'source',start:1};
function consumer(body){
  const root=mkdtempSync('rust/target/native-read-unit-');mkdirSync(join(root,'src'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/main.tsx'),code=`import {createSignal,createMemo,untrack} from 'solid-js';function App(){const [value]=createSignal(1);${body}}`;
  writeFileSync(path,code);const session=projectReadSession(root),state=session.get(path,code);
  return {path:state.source.fileName,code,state,result:transformNativeReads(code,state.source.fileName,session)};
}
test('native read model authenticates shared export identities and refuses changed bytes',()=>{
  const path=join(native,'dev-shared.js'),text=readFileSync(path,'utf8'),model=nativeReaderPremise(text,path);
  assert.equal(model.names.reader,'read');assert.equal(model.returns.length,5);
  assert.throws(()=>nativeReaderPremise(text+'\n',path),/outside the inspected profile/);
  const result=instrumentNativeReads(text,path);assert.equal(result.map.sourcesContent[0],text);assert(result.code.includes('enterIntent'));
});
test('actual native read hook covers wrappers, bound copies and receiver methods',async()=>{
  const root=mkdtempSync('rust/target/native-read-core-');cpSync(native,join(root,'dist'),{recursive:true});writeFileSync(join(root,'package.json'),'{"type":"module"}');
  for(const file of ['dev.js','dev-shared.js'])writeFileSync(join(root,'dist',file),instrumentNativeReads(readFileSync(join(native,file),'utf8'),join(native,file)).code);
  const manager=createNativeReads(),previous=globalThis.__nativeNodeReads;globalThis.__nativeNodeReads=manager;
  try{
    const solid=await import(pathToFileURL(join(process.cwd(),root,'dist/dev.js'))),object={},[get,set]=solid.createSignal(object),keys=Reflect.ownKeys(get),name=get.name;
    for(const read of [get,()=>get(),get.bind(null),()=>({read:get}).read()])assert.equal(manager.candidate(site,read),object);
    assert.equal(manager.events.length,1);assert.equal(manager.events[0].occurrences,4);assert.equal(manager.events[0].identity.kind,'native-node');
    assert.deepEqual(Reflect.ownKeys(get),keys);assert.equal(get.name,name);
    assert.equal(manager.candidate(site,()=>solid.untrack(()=>get())),object);assert.equal(manager.events[0].occurrences,4);
    const wrapper=()=>solid.untrack(()=>get());assert.equal(manager.candidate(site,wrapper),object);assert.equal(manager.events[0].occurrences,4);
    solid.createRoot(dispose=>{const memo=solid.createMemo(()=>manager.candidate(site,()=>get()));assert.equal(memo(),object);dispose();});assert.equal(manager.events[0].occurrences,4);
    const next={};set(next);solid.flush();assert.equal(manager.candidate(site,()=>get()),next);assert.equal(manager.events[0].occurrences,5);
    assert(manager.events[0].nativeRead.frames.some(frame=>frame.path.includes('/dev-shared.js')));
    const error=Error('read failure'),failing=solid.createMemo(()=>{throw error;});
    const plain=await import(pathToFileURL(join(native,'dev.js'))),plainFailing=plain.createMemo(()=>{throw error;});let originalFailure;
    try{plainFailing();}catch(error){originalFailure=error;}assert(originalFailure);
    assert.throws(()=>manager.candidate({...site,start:2},()=>failing()),e=>e.name===originalFailure.name&&e.message===originalFailure.message);assert.equal(manager.events.length,1);
    assert.equal(manager.candidate({...site,start:3},()=>9),9);assert.equal(manager.events.length,1);
  }finally{globalThis.__nativeNodeReads=previous;}
});
test('scopes and intent restore after exceptions without retaining failed calls',()=>{
  const manager=createNativeReads(),node={},fn=()=>1;manager.tag(fn,node,{});const premise={},error=Error('failed');
  assert.throws(()=>manager.candidate(site,()=>{const ticket=manager.begin(node,premise,()=>null,()=>null);manager.finish(1,ticket);throw error;}),e=>e===error);assert.equal(manager.events.length,0);
  assert.equal(manager.finish(7,manager.begin(node,premise,()=>null,()=>null)),7);assert.equal(manager.events.length,0);
  const token=manager.enterIntent();try{manager.candidate(site,()=>manager.finish(1,manager.begin(node,premise,()=>null,()=>null)));}finally{manager.leaveIntent(token);}assert.equal(manager.events.length,0);
  manager.candidate(site,()=>manager.finish(1,manager.begin(node,premise,()=>null,()=>null)));assert.equal(manager.events.length,1);
});
test('nested scopes attribute reads to the exact innermost call and preserve values',()=>{
  const manager=createNativeReads(),node={};manager.tag(()=>1,node,{});const inner={...site,start:2},value={};
  assert.equal(manager.candidate(site,()=>manager.candidate(inner,()=>manager.finish(value,manager.begin(node,{},()=>null,()=>null)))),value);
  assert.equal(manager.events.length,1);assert.deepEqual(manager.events[0].site,inner);
});
test('unknown native nodes and asynchronous work after call completion stay unobserved',async()=>{
  const manager=createNativeReads(),known={};manager.tag(()=>1,known,{});
  manager.candidate(site,()=>manager.finish(1,manager.begin({}, {},()=>null,()=>null)));assert.equal(manager.events.length,0);
  await manager.candidate(site,async()=>{await Promise.resolve();return manager.finish(1,manager.begin(known,{},()=>null,()=>null));});assert.equal(manager.events.length,0);
});
for(const [name,body]of [
  ['wrapper',`const wrapped=()=>value();return createMemo(()=>Promise.resolve().then(()=>wrapped()));`],
  ['bound copy',`const bound=value.bind(null);return createMemo(()=>Promise.resolve().then(()=>bound()));`],
  ['member',`const api={value};return createMemo(()=>Promise.resolve().then(()=>api.value()));`],
  ['receiver method',`const api={value,read(){return this.value();}};return createMemo(()=>Promise.resolve().then(()=>api.read()));`],
])test(name+' has an exact candidate with a receiver-preserving thunk',()=>{
  const input=consumer(body);assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);assert(input.result.code.includes('withNativeReadCandidate'));
  assert.equal(mayHaveNativeCallbackRead(input.code,input.path),true);assert.equal(input.result.map.sourcesContent[0],input.code);
});
for(const [name,body]of [
  ['computed member',`const api={value};return createMemo(()=>Promise.resolve().then(()=>api['value']()));`],
  ['optional member',`const api:{value?:()=>number}={value};return createMemo(()=>Promise.resolve().then(()=>api.value?.()));`],
  ['unknown argument flow',`return createMemo(()=>Promise.resolve().then(()=>Promise.resolve(value())));`],
  ['explicit untrack',`return createMemo(()=>Promise.resolve().then(()=>untrack(()=>value())));`],
])test(name+' remains open',()=>assert.equal(consumer(body).result.sites.length,0));
test('published typing error suppresses all candidates',()=>{
  const input=consumer(`value(42);return createMemo(()=>Promise.resolve().then(()=>value()));`);assert.equal(input.result.sites.length,0);assert(input.result.open[0].codes.includes(2554));
});
test('project plugin enrolls the member candidate after the syntax gate',()=>{
  const input=consumer(`const api={value};return createMemo(()=>Promise.resolve().then(()=>api.value()));`),plugin=nativeReadPlugin();
  plugin.configResolved({root:join(input.path,'../..')});const result=plugin.transform(input.code,input.path);
  assert.deepEqual(plugin.refused,[]);assert(result.code.includes('withNativeReadCandidate'));assert.equal(plugin.transformed[0].sites.length,1);
});
