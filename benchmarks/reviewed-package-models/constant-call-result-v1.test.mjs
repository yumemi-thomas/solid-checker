import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {ts} from './lower.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {constantWholeCallbackResult} from './constant-call-result-v1.mjs';
import {nativeReadFeedback} from './native-read-feedback-v9.mjs';
function input(setup,expression='read()',files={},imports=''){
  const root=resolve(mkdtempSync('rust/target/constant-result-unit-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
  for(const[name,text]of Object.entries(files))writeFileSync(join(root,'src',name),text);
  const path=join(root,'src/main.tsx'),code=`import {createMemo,createSignal} from 'solid-js';${imports}\nfunction App(){const [get]=createSignal(1,{ownedWrite:true});${setup}return createMemo(()=>Promise.resolve().then(()=>${expression}));}`;writeFileSync(path,code);
  const state=projectReadSession(root).get(path,code);return{...state,path,code,sites:nativeReadSites(state.program,state.source).sites};
}
function model(row){assert.deepEqual(row.errors.map(error=>({code:error.code,message:ts.flattenDiagnosticMessageText(error.messageText,'\n')})),[]);assert.equal(row.sites.length,1);return constantWholeCallbackResult(row.program,row.source,row.sites[0]);}
for(const[caseName,setup,expected]of[
  ['constant arrow','const read=()=>{get();return 9;};',{kind:'number',value:'9'}],
  ['constant declaration','function read(){get();return 9;}',{kind:'number',value:'9'}],
  ['constant alias','const original=()=>{get();return 9;};const read=(original satisfies ()=>number);',{kind:'number',value:'9'}],
  ['nested callback returns ignored','const read=()=>{const fn=()=>get();fn();return 9;};',{kind:'number',value:'9'}],
  ['same return branches','function read(){if(get())return 9;return 9;}',{kind:'number',value:'9'}],
  ['negative number','const read=()=>{get();return -9;};',{kind:'number',value:'-9'}],
  ['string','const read=()=>{get();return "constant";};',{kind:'string',value:'constant'}],
  ['boolean','const read=()=>{get();return false;};',{kind:'boolean',value:false}],
  ['null','const read=()=>{get();return null;};',{kind:'null',value:null}],
  ['undefined','const read=()=>{get();return;};',{kind:'undefined',value:null}],
])test(caseName+' has exact source evidence for its normal return value',()=>{
  const result=model(input(setup));assert.equal(result.kind,'source-primitive-constant-return');assert.deepEqual(result.constant,expected);assert.equal(result.authority,false);assert.equal(result.certification,false);
  assert(result.bindings.every(binding=>binding.sha256&&binding.end>binding.start));assert(result.returns.every(item=>item.span.sha256));
});
for(const[caseName,setup,expression]of[
  ['returned native value','const read=()=>get();'],
  ['mixed return values','function read(){if(get())return 9;return 10;}'],
  ['missing terminal return','function read(){if(get())return 9;}'],
  ['dynamic branch','function read(){if(get())return get();return 9;}'],
  ['returned object','const read=()=>{get();return {n:9};};'],
  ['returned array','const read=()=>{get();return [9];};'],
  ['returned function','const read=()=>{get();return ()=>9;};'],
  ['async function','const read=async()=>{get();return 9;};'],
  ['generator','function* read(){get();yield 1;return 9;}'],
  ['mutable binding','let read=()=>{get();return 9;};'],
  ['mutable alias','const original=()=>9;let read=original;'],
  ['direct eval in module','function read(){return 9;}eval("read=()=>get()");'],
  ['signed zero branches','function read(){if(get())return -0;return 0;}'],
  ['ordinary member dispatch','const api={read:()=>9};','api.read()'],
  ['larger result expression','const read=()=>{get();return 9;};let raw=1;','read()+raw'],
  ['registration wrapper','const register=()=>{get();return 9;};let raw=1;const read=()=>{register();return raw;};'],
])test(caseName+' keeps result flow open',()=>assert.equal(model(input(setup,expression)),null));
test('named and namespace imports bind the actual source function across files',()=>{
  const files={'consumer.ts':'export function consume(read:()=>number){read();return 9;}'};
  for(const[imports,expression]of[["import {consume as read} from './consumer';",'read(get)'],["import * as helpers from './consumer';",'helpers.consume(get)']]){
    const row=input('',expression,files,imports),result=model(row);assert(result.function.path.endsWith('/consumer.ts'));assert.equal(result.bindings[0].kind,'unwritten-function-declaration');
  }
});
test('a write in an exported helper is visible before suppressing an imported declaration',()=>{
  for(const assignment of ['consume=read;','({consume}={consume:read});']){
    const row=input('replace(get);','consume()',{'consumer.js':`export function consume(){return 9;}export function replace(read){${assignment}}`},"import {consume,replace} from './consumer';");assert.equal(model(row),null);
  }
});
test('external declaration without a source body stays open',()=>{
  const row=input('','read()',{'consumer.d.ts':'export declare function read():number;'},"import {read} from './consumer';");assert.equal(model(row),null);
});
test('published typing errors suppress feedback before constant analysis',()=>{
  const row=input('createSignal<boolean>(1);const read=()=>9;');assert(row.errors.some(error=>error.code===2769));const result=nativeReadFeedback(row.program,row.source,[]);assert.deepEqual(result.notes,[]);assert.deepEqual(result.suppressed,[]);
});
test('TypeScript owns assignment to a TypeScript function declaration',()=>{
  const row=input('function read(){return 9;}read=()=>get();');assert(row.errors.some(error=>error.code===2630));const result=nativeReadFeedback(row.program,row.source,[]);assert.deepEqual(result.notes,[]);assert.deepEqual(result.suppressed,[]);
});
