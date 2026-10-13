import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {ts} from './lower.mjs';
import {projectReadSession} from './project-read-session-v2.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {asyncConstantWholeCallbackResult} from './async-constant-result-v1.mjs';
function input(setup,expression='read()',files={},imports=''){
  const root=resolve(mkdtempSync('rust/target/async-constant-result-unit-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
  for(const[name,text]of Object.entries(files))writeFileSync(join(root,'src',name),text);
  const path=join(root,'src/main.tsx'),code=`import {createMemo,createSignal} from 'solid-js';${imports}\nfunction App(){const [get]=createSignal(1,{ownedWrite:true});${setup}return createMemo(()=>Promise.resolve().then(()=>${expression}));}`;writeFileSync(path,code);
  const state=projectReadSession(root).get(path,code);return{...state,path,code,sites:nativeReadSites(state.program,state.source).sites};
}
function model(row){assert.deepEqual(row.errors.map(error=>({code:error.code,message:ts.flattenDiagnosticMessageText(error.messageText,'\n')})),[]);assert.equal(row.sites.length,1);return asyncConstantWholeCallbackResult(row.program,row.source,row.sites[0]);}
const nine={kind:'number',value:'9'};
for(const[name,setup,expected=nine]of[
  ['delayed constant','async function read(){await Promise.resolve();get();return 9;}'],
  ['two awaits','async function read(){await Promise.resolve();await Promise.resolve();get();return 9;}'],
  ['async arrow','const read=async()=>{await Promise.resolve();get();return 9;};'],
  ['await literal','async function read(){get();return await 9;}'],
  ['both branches','async function read(){await Promise.resolve();if(get())return 9;else return 9;}'],
  ['throwing branch','async function read(){await Promise.resolve();if(get())throw Error("expected");return 9;}'],
  ['finally replaces unknown return','async function read(){await Promise.resolve();try{return get();}finally{return 9;}}'],
  ['nested finally replacement','async function read(){await Promise.resolve();try{try{return get();}finally{return 10;}}finally{return 9;}}'],
  ['conditional finally agrees','async function read(){await Promise.resolve();try{get();return 9;}finally{if(get())return 9;}}'],
  ['normal finally preserves result','async function read(){await Promise.resolve();try{get();return 9;}finally{get();}}'],
  ['catch agrees','async function read(){await Promise.resolve();try{get();return 9;}catch{return 9;}}'],
  ['catch handles definite throw','async function read(){await Promise.resolve();try{throw Error("expected");}catch{get();return 9;}}'],
  ['nested returns stay local','async function read(){const nested=()=>get();await Promise.resolve();nested();return 9;}'],
  ['finally overrides unknown loop','async function read(){await Promise.resolve();try{while(get()){return get();}}finally{return 9;}}'],
  ['explicit and implicit undefined','async function read(){await Promise.resolve();if(get())return;}',{kind:'undefined',value:null}],
  ['negative zero','async function read(){await Promise.resolve();get();return -0;}',{kind:'number',value:'-0'}],
  ['string','async function read(){await Promise.resolve();get();return "constant";}',{kind:'string',value:'constant'}],
  ['boolean','async function read(){await Promise.resolve();get();return false;}',{kind:'boolean',value:false}],
  ['null','async function read(){await Promise.resolve();get();return null;}',{kind:'null',value:null}],
  ['bigint','async function read(){await Promise.resolve();get();return 9n;}',{kind:'bigint',value:'9'}],
  ['expression arrow','const read=async()=>false;',{kind:'boolean',value:false}],
])test(name+' has bounded constant fulfilled-value evidence',()=>{const result=model(input(setup));assert.equal(result.scope,'normal-fulfilled-value-only');assert.deepEqual(result.constant,expected);assert.equal(result.authority,false);assert.equal(result.certification,false);});
for(const[name,setup,expression='read()']of[
  ['read result','async function read(){await Promise.resolve();return get();}'],
  ['catch-only constant','async function read(){await Promise.resolve();try{return get();}catch{return 9;}}'],
  ['conditional finally override','async function read(){await Promise.resolve();try{return get();}finally{if(get())return 9;}}'],
  ['finally reads result','async function read(){await Promise.resolve();try{return 9;}finally{return get();}}'],
  ['different branches','async function read(){await Promise.resolve();if(get())return 9;return 10;}'],
  ['fallthrough differs','async function read(){await Promise.resolve();if(get())return 9;}'],
  ['signed zero differs','async function read(){await Promise.resolve();if(get())return -0;return 0;}'],
  ['generator','async function* read(){await Promise.resolve();get();yield 9;}'],
  ['synchronous helper','function read(){get();return 9;}'],
  ['mutable binding','let read=async()=>{get();return 9;};'],
  ['mutable alias','const original=async()=>9;let read=original;'],
  ['ordinary member','const api={read:async()=>9};','api.read()'],
  ['unknown loop','async function read(){await Promise.resolve();while(get()){return get();}return 9;}'],
  ['switch','async function read(){await Promise.resolve();switch(get()){case 1:return 9;default:return 9;}}'],
  ['constant variable unresolved','async function read(){const nine=9;await Promise.resolve();get();return nine;}'],
  ['object result','async function read(){await Promise.resolve();return {value:9};}'],
  ['Promise adoption','async function read(){get();return Promise.resolve(9);}'],
  ['larger callback result','async function read(){await Promise.resolve();get();return 9;}','[read()]'],
  ['direct eval','async function read(){return 9;}eval("read=()=>get()");'],
])test(name+' keeps result flow open',()=>assert.equal(model(input(setup,expression)),null));
test('exact imported and namespace helpers include source identity',()=>{
  const files={'helper.ts':'export async function consume(read:()=>number){await Promise.resolve();try{return read();}finally{return 9;}}'};
  for(const[imports,expression]of[["import {consume as read} from './helper';",'read(get)'],["import * as helper from './helper';",'helper.consume(get)']]){const result=model(input('',expression,files,imports));assert(result.function.path.endsWith('/helper.ts'));assert.equal(result.bindings[0].kind,'unwritten-function-declaration');assert.deepEqual(result.constant,nine);}
});
test('a transparent const alias preserves the actual imported helper',()=>{const result=model(input('const read=(consume satisfies (read:()=>number)=>Promise<number>);','read(get)',{'helper.ts':'export async function consume(read:()=>number){await Promise.resolve();read();return 9;}'},"import {consume} from './helper';"));assert.equal(result.bindings.length,2);assert.deepEqual(result.constant,nine);});
test('declaration-only helpers remain open',()=>assert.equal(model(input('','read()',{'helper.d.ts':'export declare function read():Promise<number>;'},"import {read} from './helper';")),null));
test('a valid JavaScript write to the source function prevents trust',()=>{const row=input('replace(get);','read()',{'helper.js':'export async function read(){return 9;}export function replace(get){read=async()=>get();}'},"import {read,replace} from './helper';");assert.equal(model(row),null);});
test('statement budget exhaustion remains open',()=>{const row=input('async function read(){'+Array.from({length:260},()=> 'get();').join('')+'return 9;}');assert.equal(model(row),null);});
