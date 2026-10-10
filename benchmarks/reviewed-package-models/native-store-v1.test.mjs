import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,symlinkSync,cpSync} from 'node:fs';
import {join} from 'node:path';
import {pathToFileURL} from 'node:url';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {createNativeReads} from './native-read-runtime-v2.mjs';
import {instrumentNativeReads} from './native-read-hook-v2.mjs';
import {nativeStorePremise} from './native-store-premise-v1.mjs';
import pluginFactory,{transformNativeReads} from './async-read-transform-v7.mjs';
import {nativeReadFeedback} from './native-read-feedback-v2.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {mayHaveNativeCallbackRead} from './async-read-prefilter-v3.mjs';
const install=new URL('../../rust/target/app-import-metric/apps/helge-dev',import.meta.url).pathname,native=join(install,'node_modules/@solidjs/signals/dist'),core=join(native,'dev.js'),coreText=readFileSync(core,'utf8');
const site={path:'consumer',sourceSha256:'source',start:1};
function consumer(body){const root=mkdtempSync('rust/target/native-store-unit-');mkdirSync(join(root,'src'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/main.tsx'),code=`import {createStore,createMemo,untrack} from 'solid-js';function App(){const [state]=createStore({n:1,user:{n:1},list:[1]});${body}}`;writeFileSync(path,code);
  const session=projectReadSession(root),state=session.get(path,code);return {root,path:state.source.fileName,code,state,result:transformNativeReads(code,state.source.fileName,session)};}
test('store model binds the constructor target, Proxy handler and own-data reader',()=>{
  const model=nativeStorePremise(coreText,core);assert.equal(model.kind,'native-store-target-and-data-read');assert.equal(model.names.reader,'serveDataKey');assert.equal(model.returns.length,7);
  assert.throws(()=>nativeStorePremise(coreText+'\n',core),/outside the inspected profile/);
  assert.equal(instrumentNativeReads(coreText,core).map.sourcesContent[0],coreText);
});
test('actual native store reads preserve Proxy shape, nested arrays, values and tracking',async()=>{
  const root=mkdtempSync('rust/target/native-store-core-');cpSync(native,join(root,'dist'),{recursive:true});writeFileSync(join(root,'package.json'),'{"type":"module"}');
  for(const file of ['dev.js','dev-shared.js'])writeFileSync(join(root,'dist',file),instrumentNativeReads(readFileSync(join(native,file),'utf8'),join(native,file)).code);
  const manager=createNativeReads(),previous=globalThis.__nativeNodeReads;globalThis.__nativeNodeReads=manager;
  try{
    const solid=await import(pathToFileURL(join(process.cwd(),root,'dist/dev.js'))),[state,set]=solid.createStore({n:1,list:[1]}),keys=Reflect.ownKeys(state),original=state;
    assert.equal(manager.candidate(site,()=>state.n),1);assert.equal(manager.events.length,1);assert.equal(manager.events[0].storeKey,'n');assert.equal(manager.events[0].identity.kind,'native-store-target');
    assert.equal(manager.candidate(site,()=>state.n),1);assert.equal(manager.events[0].occurrences,2);
    assert.equal(manager.candidate(site,()=>solid.untrack(()=>state.n)),1);assert.equal(manager.events[0].occurrences,2);
    const array=state.list;assert(Array.isArray(array));assert.equal(manager.candidate({...site,start:2},()=>state.list.length),1);assert.equal(state.list,array);
    assert.equal(manager.events.filter(event=>event.site.start===2).length,2);
    solid.createRoot(dispose=>{const memo=solid.createMemo(()=>manager.candidate({...site,start:3},()=>state.n));assert.equal(memo(),1);dispose();});assert.equal(manager.events.some(event=>event.site.start===3),false);
    set(draft=>{draft.n=2;draft.list.push(2);});solid.flush();assert.equal(manager.candidate(site,()=>state.n),2);assert.equal(state,original);assert.deepEqual(Reflect.ownKeys(state),keys);
    assert.equal(state.list.length,2);
  }finally{globalThis.__nativeNodeReads=previous;}
});
test('unknown targets and symbol/then probes remain unobserved',()=>{
  const manager=createNativeReads(),target={};manager.tagStore(target,{});assert.equal(manager.tagStore(target,{}),target);
  for(const [object,key]of [[{},'n'],[target,Symbol('n')],[target,'then']])manager.candidate(site,()=>manager.finish(1,manager.beginStore(object,key,{},()=>null,()=>null)));
  assert.equal(manager.events.length,0);
});
for(const expression of ['state.n','state.user.n','state.list.length','({n:state.n})','state.n+1'])test('declared property '+expression+' has a current expression witness',()=>{
  const input=consumer(`return createMemo(()=>Promise.resolve().then(()=>${expression}));`);assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);
  assert.equal(input.result.sites[0].kind,'native-property-candidate');assert.equal(mayHaveNativeCallbackRead(input.code,input.path),true);assert.equal(input.result.map.sourcesContent[0],input.code);
});
for(const [name,body]of [
  ['computed key',`return createMemo(()=>Promise.resolve().then(()=>state['n']));`],
  ['optional receiver',`const maybe:typeof state|undefined=state;return createMemo(()=>Promise.resolve().then(()=>maybe?.n));`],
  ['discarded property',`return createMemo(()=>Promise.resolve().then(()=>{state.n;return 9;}));`],
  ['explicit untrack',`return createMemo(()=>Promise.resolve().then(()=>untrack(()=>state.n)));`],
  ['unknown argument flow',`return createMemo(()=>Promise.resolve().then(()=>Promise.resolve(state.n)));`],
])test(name+' remains open',()=>assert.equal(consumer(body).result.sites.length,0));
test('a callee property is not separately evaluated by instrumentation',()=>{
  const input=consumer(`const api={read(){return state.n;}};return createMemo(()=>Promise.resolve().then(()=>api.read()));`);assert.equal(input.result.sites.length,1);assert.equal(input.result.sites[0].kind,'native-read-candidate');
});
test('plain getter receivers remain candidates requiring observed native evidence',()=>{
  const input=consumer(`const plain={get n(){return 9;}};return createMemo(()=>Promise.resolve().then(()=>plain.n));`);assert.equal(input.result.sites.length,1);
  assert.equal(nativeReadFeedback(input.state.program,input.state.source,[]).notes.length,0);
});
test('published missing-property error suppresses the entire input',()=>{
  const input=consumer(`return createMemo(()=>Promise.resolve().then(()=>state.missing));`);assert.equal(input.result.sites.length,0);assert(input.result.open[0].codes.includes(2339));
});
test('property-only source passes the necessary gate and project plugin',()=>{
  const input=consumer(`return createMemo(()=>Promise.resolve().then(()=>state.n));`),plugin=pluginFactory();plugin.configResolved({root:join(input.path,'../..')});
  assert(plugin.transform(input.code,input.path).code.includes('withNativeReadCandidate'));assert.deepEqual(plugin.refused,[]);
});
test('feedback authenticates store construction, serving entry and current property span',()=>{
  const input=consumer(`return createMemo(()=>Promise.resolve().then(()=>state.n));`),selected=input.result.sites[0],model=nativeStorePremise(coreText,core),source=ts.createSourceFile(core,coreText,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),position=source.getLineAndCharacterOfPosition(model.entry.start);
  const event={site:selected,identity:{id:1,kind:'native-store-target',premise:model,originalCreationFrames:[{path:core,line:model.line,column:model.column,sourceSha256:hash(coreText)}]},
    nativeRead:{premise:model,originalFrames:[{path:core,line:position.line+1,column:position.character+1,sourceSha256:hash(coreText)}]},storeKey:'n',context:{observer:false,owner:false},occurrences:1,
    originalFrames:[{path:input.path,line:selected.line,column:selected.column,sourceSha256:hash(input.code)}]};
  const evaluate=event=>nativeReadFeedback(input.state.program,input.state.source,[event]);assert.equal(evaluate(event).notes.length,1);assert.equal(evaluate(event).notes[0].severity,'info');
  assert.equal(evaluate({...event,identity:{...event.identity,originalCreationFrames:[]}}).notes.length,0);
  assert.equal(evaluate({...event,nativeRead:{...event.nativeRead,originalFrames:[]}}).notes.length,0);
  assert.equal(evaluate({...event,site:{...selected,sourceSha256:'changed'}}).notes.length,0);
  assert.equal(evaluate({...event,storeKey:'then'}).notes.length,0);
});
