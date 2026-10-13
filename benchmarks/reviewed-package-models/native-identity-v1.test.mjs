import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,symlinkSync,cpSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash} from './catalog.mjs';
import {createNativeIdentities} from './native-identity-runtime-v1.mjs';
import {instrumentNativeAccessor,nativeAccessorPremise,nativeAccessorHook} from './native-accessor-hook-v1.mjs';
import {transformIdentityReads} from './async-read-transform-v5.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {nativeIdentityFeedback} from './native-identity-feedback-v1.mjs';
const install=new URL('../../rust/target/app-import-metric/apps/helge-dev',import.meta.url).pathname,
  native=join(install,'node_modules/@solidjs/signals/dist/dev.js'),nativeText=readFileSync(native,'utf8');
function consumer(body,files={}){
  const root=mkdtempSync('rust/target/native-identity-unit-');mkdirSync(join(root,'src'));
  symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/main.tsx'),code=`import {createSignal,createMemo,untrack} from 'solid-js';${body}`;
  writeFileSync(path,code);for(const [name,text]of Object.entries(files))writeFileSync(join(root,'src',name),text);
  const session=projectReadSession(root),state=session.get(path,code);
  return {root,path:state.source.fileName,code,session,state,result:transformIdentityReads(code,state.source.fileName,session)};
}
test('exact native hook retains source maps and refuses changed core bytes',()=>{
  const result=instrumentNativeAccessor(nativeText,native);assert.equal(result.map.sourcesContent[0],nativeText);
  assert.equal(nativeAccessorHook().transform(nativeText,native).map.sourcesContent[0],nativeText);
  assert.throws(()=>nativeAccessorPremise(nativeText+'\n',native),/outside the inspected profile/);
});
test('native getter objects, properties, result identities and contexts survive',async()=>{
  const root=mkdtempSync('rust/target/native-identity-core-');cpSync(join(install,'node_modules/@solidjs/signals/dist'),join(root,'dist'),{recursive:true});
  writeFileSync(join(root,'package.json'),'{"type":"module"}');writeFileSync(join(root,'dist/dev.js'),instrumentNativeAccessor(nativeText,native).code);
  const manager=createNativeIdentities(),previous=globalThis.__nativeAccessorIdentities;globalThis.__nativeAccessorIdentities=manager;
  try{
    const solid=await import(pathToFileURL(join(process.cwd(),root,'dist/dev.js')));
    const value={n:1},[get,set]=solid.createSignal(value),keys=Reflect.ownKeys(get),name=get.name;
    assert.equal(manager.tag(get,{},solid.getObserver,solid.getOwner),get);assert.equal(manager.taggedCount,1);
    const site={path:'consumer',sourceSha256:'source',start:1};assert.equal(manager.call(get,site),value);
    assert.deepEqual(Reflect.ownKeys(get),keys);assert.equal(get.name,name);assert.equal(manager.events.length,1);
    assert(manager.events[0].identity.creationFrames.some(frame=>frame.path.includes('/dist/dev.js')));
    solid.createRoot(dispose=>{const memo=solid.createMemo(()=>manager.call(get,site));assert.equal(memo(),value);dispose();});
    assert.equal(manager.events.length,1);set({n:2});solid.flush();assert.equal(manager.call(get,site).n,2);
    assert.equal(manager.events[0].occurrences,2);
  }finally{globalThis.__nativeAccessorIdentities=previous;}
});
test('unknown functions execute once, return their original object and stay quiet',()=>{
  const manager=createNativeIdentities(),value={},site={path:'x',start:0};let calls=0;
  assert.equal(manager.call(()=>{calls++;return value;},site),value);assert.equal(calls,1);assert.equal(manager.events.length,0);
  const error=Error('original');assert.throws(()=>manager.call(()=>{throw error;},site),e=>e===error);
});
test('contexts come from the tagged runtime instance and are sampled before read',()=>{
  const manager=createNativeIdentities();let owner=null,observer=null;
  const get=()=>{owner={};return 1;};manager.tag(get,{},()=>observer,()=>owner);
  manager.call(get,{path:'x',start:1});assert.deepEqual(manager.events[0].context,{owner:false,observer:false});
  manager.call(get,{path:'x',start:1});assert.equal(manager.events[0].occurrences,1);
});
for(const [name,body,files]of [
  ['mutable alias',`function App(){const [value]=createSignal(1);let alias=value;return createMemo(()=>Promise.resolve().then(()=>alias()));}`,{}],
  ['cross-file getter',`import {value} from './shared';function App(){return createMemo(()=>Promise.resolve().then(()=>value()));}`,{'shared.ts':`import {createSignal} from 'solid-js';export const [value]=createSignal(1);`}],
  ['cross-file factory',`import {make} from './shared';function App(){const value=make();return createMemo(()=>Promise.resolve().then(()=>value()));}`,{'shared.ts':`import {createSignal} from 'solid-js';export function make(){return createSignal(1)[0];}`}],
  ['unknown function candidate',`function App(){const value=()=>9;return createMemo(()=>Promise.resolve().then(()=>value()));}`,{}],
])test(name+' is a candidate requiring runtime identity',()=>{
  const input=consumer(body,files);assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);
  assert.equal(input.result.sites[0].staticDispatch,'open');assert(input.result.code.includes('callNativeCandidate'));
});
for(const [name,body]of [
  ['explicit untrack',`return createMemo(()=>Promise.resolve().then(()=>untrack(()=>value())));`],
  ['unknown argument flow',`const identity=(n:number)=>n;return createMemo(()=>Promise.resolve().then(()=>identity(value())));`],
  ['unused result',`return createMemo(()=>Promise.resolve().then(()=>{const inspected=(()=>value())();return 9;}));`],
])test(name+' stays open',()=>assert.equal(consumer(`function App(){const [value]=createSignal(1);${body}}`).result.sites.length,0));
test('real typing error is excluded before identity instrumentation',()=>{
  const result=consumer(`function App(){const [value]=createSignal(1);value(42);return createMemo(()=>Promise.resolve().then(()=>value()));}`).result;
  assert.equal(result.sites.length,0);assert(result.open[0].codes.includes(2554));
});
test('feedback authenticates native creation and current read frames',()=>{
  const input=consumer(`function App(){const [value]=createSignal(1);return createMemo(()=>Promise.resolve().then(()=>value()));}`),site=input.result.sites[0],premise=nativeAccessorPremise(nativeText,native);
  const event={site,identity:{id:1,premise,originalCreationFrames:[{path:native,line:premise.line,column:premise.column,sourceSha256:hash(nativeText)}]},
    context:{owner:false,observer:false},occurrences:1,originalFrames:[{path:input.path,line:site.line,column:site.column,sourceSha256:hash(input.code)}]};
  const evaluate=e=>nativeIdentityFeedback(input.state.program,input.state.source,[e]);
  assert.equal(evaluate(event).notes.length,1);assert.equal(evaluate(event).notes[0].severity,'info');
  assert.equal(evaluate({...event,identity:{...event.identity,originalCreationFrames:[]}}).notes.length,0);
  assert.equal(evaluate({...event,originalFrames:[]}).notes.length,0);
  assert.equal(evaluate({...event,site:{...site,sourceSha256:'changed'}}).notes.length,0);
  assert.equal(evaluate({...event,context:{owner:true,observer:false}}).notes.length,0);
});
