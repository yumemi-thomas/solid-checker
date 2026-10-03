import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,writeFileSync,readFileSync,symlinkSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash} from './catalog.mjs';
import {ts} from './lower.mjs';
import {packageShortcuts,instrumentPackageShortcuts} from './package-shortcut-v1.mjs';
import {createNativeReads} from './native-read-runtime-v3.mjs';
import {nativeReadFeedback} from './native-read-feedback-v3.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformNativeReads} from './async-read-transform-v8.mjs';
const install=resolve('rust/target/app-import-metric/apps/helge-dev'),site={path:'consumer',sourceSha256:'source',start:1};
function input(text){const root=resolve(mkdtempSync('rust/target/package-shortcut-unit-'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');writeFileSync(join(root,'package.json'),'{"type":"module"}');const path=join(root,'index.js');writeFileSync(path,text);return {root,path,text};}
const head=`import {getObserver,createSignal} from 'solid-js';`;
for(const [label,body,count]of [
  ['alias',`import {getObserver as observer,createSignal as source} from 'solid-js';function read(){if(!observer())return 1;const [value]=source(1);return value();}`,1],
  ['block return',head+`function read(){if(!getObserver()){return 1;}const [value]=createSignal(1);return value();}`,1],
  ['shadowed observer',head+`function read(getObserver){if(!getObserver())return 1;const [value]=createSignal(1);return value();}`,0],
  ['shadowed source',head+`function read(createSignal){if(!getObserver())return 1;const [value]=createSignal(1);return value();}`,0],
  ['no source',head+`function read(){if(!getObserver())return 1;return 2;}`,0],
  ['nested source',head+`function read(){if(!getObserver())return 1;function nested(){return createSignal(1);}return nested();}`,0],
  ['source before guard',head+`function read(){const [value]=createSignal(1);if(!getObserver())return 1;return value();}`,0],
  ['async body',head+`async function read(){if(!getObserver())return 1;const [value]=createSignal(1);return value();}`,0],
  ['generator body',head+`function* read(){if(!getObserver())return 1;const [value]=createSignal(1);yield value();}`,0],
  ['multi-statement branch',head+`function read(){if(!getObserver()){console.log('read');return 1;}const [value]=createSignal(1);return value();}`,0],
  ['non-negated observer',head+`function read(){if(getObserver())return 1;const [value]=createSignal(1);return value();}`,0],
  ['arbitrary observer',`function getObserver(){return null;}function createSignal(){return [()=>1];}function read(){if(!getObserver())return 1;const [value]=createSignal();return value();}`,0],
])test('shortcut enrollment handles '+label,()=>{const source=input(body),parsed=packageShortcuts(source.text,source.path);assert.equal(parsed.models.length,count);});
test('real static-store and trigger-cache models carry exact published symbols',()=>{
  for(const path of ['/private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-e1VMl0/node_modules/@solid-primitives/trigger/dist/index.js','/private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-0SWsjs/node_modules/@solid-primitives/static-store/dist/index.js']){
    const text=readFileSync(path,'utf8'),models=packageShortcuts(text,path).models;assert.equal(models.length,1);assert(models[0].observerDeclarations.length);assert(models[0].signalDeclarations[0].length);assert.equal(models[0].valueFlow,'open');assert.equal(instrumentPackageShortcuts(text,path).map.sourcesContent[0],text);
  }
});
test('transformation preserves receiver, arguments, getter count and thrown return expression',async()=>{
  const original=head+`export function read(key){if(!getObserver())return this[key];const [value]=createSignal(1);return value();}`;
  const source=input(original),result=instrumentPackageShortcuts(original,source.path);writeFileSync(join(source.root,'observed.js'),result.code);
  const {read}=await import(pathToFileURL(join(source.root,'observed.js'))),manager=createNativeReads(),previous=globalThis.__nativeNodeReads;globalThis.__nativeNodeReads=manager;
  try{let calls=0;const object={get n(){calls++;return 7;}};assert.equal(manager.candidate(site,()=>read.call(object,'n')),7);assert.equal(calls,1);assert.equal(manager.events.length,1);
    const error=new Error('return failed');assert.throws(()=>manager.candidate({...site,start:2},()=>read.call({get n(){throw error;}},'n')),value=>value===error);assert.equal(manager.events.length,1);
  }finally{globalThis.__nativeNodeReads=previous;}
});
test('void shortcut returns remain undefined and repeated events coalesce',()=>{
  const manager=createNativeReads(),model={path:'package',returned:{start:1}};
  for(let i=0;i<2;i++)assert.equal(manager.candidate(site,()=>manager.finish(undefined,manager.beginGuard(model,()=>null))),undefined);
  assert.equal(manager.events.length,1);assert.equal(manager.events[0].occurrences,2);assert.equal(manager.events[0].identity.kind,'package-observer-guard');
});
test('absent scope, present owner and explicit native intent stay quiet',()=>{
  const manager=createNativeReads();manager.finish(1,manager.beginGuard({},()=>null));manager.candidate(site,()=>manager.finish(1,manager.beginGuard({},()=>({}))));
  const previous=manager.enterIntent();try{manager.candidate(site,()=>manager.finish(1,manager.beginGuard({},()=>null)));}finally{manager.leaveIntent(previous);}
  assert.equal(manager.events.length,0);
});
test('failed enclosing expression discards a successful inner shortcut ticket',()=>{
  const manager=createNativeReads();assert.throws(()=>manager.candidate(site,()=>{manager.finish(1,manager.beginGuard({},()=>null));throw Error('consumer failed');}));assert.equal(manager.events.length,0);
  manager.candidate({...site,start:2},()=>manager.finish(1,manager.beginGuard({},()=>null)));assert.equal(manager.events.length,1);
});
test('nested consumer scopes retain their own shortcut attribution',()=>{
  const manager=createNativeReads();manager.candidate(site,()=>{manager.candidate({...site,start:2},()=>manager.finish(1,manager.beginGuard({},()=>null)));return manager.finish(2,manager.beginGuard({},()=>null));});assert.deepEqual(manager.events.map(event=>event.site.start),[2,1]);
});
function feedbackInput(){
  const root=resolve(mkdtempSync('rust/target/package-shortcut-consumer-'));mkdirSync(join(root,'src'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/main.tsx'),code=`import {createStore,createMemo} from 'solid-js';const [state]=createStore({n:1});const result=createMemo(()=>Promise.resolve().then(()=>state.n));`;writeFileSync(path,code);
  const session=projectReadSession(root),state=session.get(path,code),transformed=transformNativeReads(code,path,session);assert.equal(state.errors.length,0);assert.equal(transformed.sites.length,1);
  const packagePath='/private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-0SWsjs/node_modules/@solid-primitives/static-store/dist/index.js',text=readFileSync(packagePath,'utf8'),model=packageShortcuts(text,packagePath).models[0],source=ts.createSourceFile(packagePath,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS);
  const position=source.getLineAndCharacterOfPosition(model.returned.start),witness=transformed.sites[0];return {state,event:{site:witness,identity:{kind:'package-observer-guard',premise:model},nativeRead:{originalFrames:[{path:packagePath,line:position.line+1,column:position.character+1,sourceSha256:hash(text)}]},context:{owner:false,observer:false},occurrences:1,originalFrames:[{path,line:witness.line,column:witness.column,sourceSha256:hash(code)}]}};
}
test('feedback authenticates the original package branch and consumer expression',()=>{
  const {state,event}=feedbackInput(),result=nativeReadFeedback(state.program,state.source,[event]);assert.equal(result.notes.length,1);assert.equal(result.notes[0].channel,'observed-package-observer-shortcut');assert.equal(result.notes[0].severity,'info');assert.equal(result.notes[0].category,'intent-open');assert.equal(result.notes[0].guardShortcut.valueFlow,'open');
});
for(const [label,mutate]of [['missing branch frame',e=>e.nativeRead.originalFrames=[]],['changed source premise',e=>e.identity.premise.sourceSha256='changed'],['missing consumer frame',e=>e.originalFrames=[]]])test('feedback refuses '+label,()=>{
  const {state,event}=feedbackInput();mutate(event);assert.equal(nativeReadFeedback(state.program,state.source,[event]).notes.length,0);
});
