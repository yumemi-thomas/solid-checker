import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,symlinkSync,cpSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {ts} from './lower.mjs';
import {createNativeReads,nativeReads} from './native-read-runtime-v3.mjs';
import {instrumentNativeReads} from './native-read-hook-v3.mjs';
import {transformNativeReads} from './async-read-transform-v10.mjs';
import {nativeReadFeedback} from './native-read-feedback-v6.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {mayHaveNativeCallbackRead} from './async-read-prefilter-v4.mjs';
const install=resolve('rust/target/app-import-metric/apps/helge-dev');
function consumer(body){
  const root=mkdtempSync('rust/target/argument-read-unit-');mkdirSync(join(root,'src'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');
  const path=resolve(root,'src/main.tsx'),code=`import {createMemo,createSignal,untrack} from 'solid-js';${body}`;writeFileSync(path,code);
  const session=projectReadSession(resolve(root)),state=session.get(path,code);return{root,path,code,state,result:transformNativeReads(code,path,session)};
}
for(const expression of ['read(1)','api.read(1,2)','read(...tuple)','read((1 satisfies number))'])test('arguments remain in the original call: '+expression,()=>{
  const input=consumer(`function App(){const [get]=createSignal(1);const read=(n:number)=>get()+n,api={read(a:number,b:number){return get()+a+b}},tuple:[number]=[1];return createMemo(()=>Promise.resolve().then(()=>${expression}));}`);
  assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);const site=input.result.sites[0];assert(site.arguments.length>0);assert.equal(site.staticDispatch,'open');
  assert.equal(mayHaveNativeCallbackRead(input.code,input.path),true);assert.equal(input.result.map.sourcesContent[0],input.code);
  assert.equal(nativeReadFeedback(input.state.program,input.state.source,[]).notes.length,0);
});
for(const[name,body]of[
  ['await argument',`function App(){const read=(n:number)=>n;return createMemo(()=>Promise.resolve().then(async()=>read(await Promise.resolve(1))));}`],
  ['yield argument',`function App(){const read=(n:number)=>n;return createMemo(()=>function*(){return read(yield 1);});}`],
  ['optional call',`function App(){const read:((n:number)=>number)|undefined=(n)=>n;return createMemo(()=>Promise.resolve().then(()=>read?.(1)));}`],
  ['computed member',`function App(){const api={read:(n:number)=>n};return createMemo(()=>Promise.resolve().then(()=>api['read'](1)));}`],
  ['explicit intent',`function App(){const [get]=createSignal(1);const read=(n:number)=>get()+n;return createMemo(()=>Promise.resolve().then(()=>untrack(()=>read(1))));}`],
  ['unused callback result',`function App(){const [get]=createSignal(1);const read=(n:number)=>get()+n;return createMemo(()=>{Promise.resolve().then(()=>read(1));return 9;});}`],
])test(name+' stays open',()=>{
  const input=consumer(body);assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,0);
});
test('an async callback argument can be created without suspending the observation thunk',()=>{
  const input=consumer(`function App(){const read=(fn:()=>Promise<number>)=>fn();return createMemo(()=>Promise.resolve().then(()=>read(async()=>{await Promise.resolve();return 1;})));}`);
  assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);
});
test('real Solid typings exclude a wrong argument before observation',()=>{
  const input=consumer(`function App(){const [get]=createSignal(1);const read=(n:number)=>get()+n;return createMemo(()=>Promise.resolve().then(()=>read('bad')));}`);
  assert(input.state.errors.some(error=>error.code===2345));assert.equal(input.result.sites.length,0);
});
test('whole call preserves member lookup, this, spread iteration, arguments and native reads',async()=>{
  const input=consumer(`export function make(log:string[]){const [get]=createSignal(1,{ownedWrite:true});const api={n:10,get read(){log.push('lookup');return function(this:{n:number},a:number,b:number){log.push('body');return this.n+get()+a+b;}}};const argument=()=>{log.push('argument');return 2;};const tuple=()=>{log.push('spread');return [3] as [number];};return createMemo(()=>()=>api.read(argument(),...tuple()));}`);
  assert.deepEqual(input.state.errors,[]);assert.equal(input.result.sites.length,1);
  const root=resolve(mkdtempSync('rust/target/argument-read-runtime-')),native=join(install,'node_modules/@solidjs/signals/dist');cpSync(native,join(root,'dist'),{recursive:true});writeFileSync(join(root,'package.json'),'{"type":"module"}');
  for(const file of ['dev.js','dev-shared.js'])writeFileSync(join(root,'dist',file),instrumentNativeReads(readFileSync(join(native,file),'utf8'),join(native,file)).code);
  const coreUrl=pathToFileURL(join(root,'dist/dev.js')).href,runtimeUrl=pathToFileURL(resolve('benchmarks/reviewed-package-models/native-read-runtime-v3.mjs')).href;
  const plain=ts.transpileModule(input.code,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText;
  for(const[name,text]of[['plain',plain],['reads',input.result.code]])writeFileSync(join(root,name+'.mjs'),text.replaceAll('"solid-js"',JSON.stringify(coreUrl)).replaceAll("'solid-js'",JSON.stringify(coreUrl)).replaceAll(JSON.stringify('/@fs'+resolve('benchmarks/reviewed-package-models/native-read-runtime-v3.mjs')),JSON.stringify(runtimeUrl)));
  const core=await import(coreUrl),manager=nativeReads,previous=globalThis.__nativeNodeReads;globalThis.__nativeNodeReads=manager;
  try{
    for(const name of ['plain','reads']){const module=await import(pathToFileURL(join(root,name+'.mjs'))),log=[];let run,dispose;core.createRoot(d=>{dispose=d;run=module.make(log)();});assert.equal(run(),16);assert.deepEqual(log,['lookup','argument','spread','body']);dispose();}
    assert.equal(manager.events.length,1);assert.equal(manager.events[0].site.arguments.length,2);assert.deepEqual(manager.events[0].context,{owner:false,observer:false});
  }finally{globalThis.__nativeNodeReads=previous;}
});
test('argument throws discard partial reads and deferred callbacks escape the synchronous scope',()=>{
  const manager=createNativeReads(),node={},premise={};manager.tag(()=>1,node,premise);const site={start:1};
  manager.candidate(site,()=>manager.finish(1,manager.begin(node,premise,()=>null,()=>null)));assert.equal(manager.events.length,1);
  const before=manager.events[0].occurrences;
  assert.throws(()=>manager.candidate(site,()=>{manager.finish(1,manager.begin(node,premise,()=>null,()=>null));throw Error('argument failed');}),/argument failed/);assert.equal(manager.events[0].occurrences,before);
  let deferred;manager.candidate(site,()=>{deferred=()=>manager.finish(1,manager.begin(node,premise,()=>null,()=>null));return Promise.resolve();});assert.equal(deferred(),1);assert.equal(manager.events[0].occurrences,before);
});
