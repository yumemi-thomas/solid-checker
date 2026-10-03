// Fresh challenges after sealing the native reader and intent observer.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head=`import {createSignal,createMemo,Loading,untrack,flush} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function observe(initial,desired){return async page=>{
  await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);
  await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);
};}
function add(id,imports,body,{name='controlled-signal',files={},initial='1',desired='2',role='target',pair=id,expectedTypingCode=null}={}){
  const packageName='@solid-primitives/'+name,project=retained.find(row=>row.package===packageName).retainedArtifacts.projectDir;
  cases.push({id:'native-read-'+id,package:packageName,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:observe(initial,desired),provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'native-read-call-scope'}});
}
const setups=[
  ['boolean',`import {createControllableBooleanSignal} from '@solid-primitives/controlled-signal';`,`const [value,set]=createControllableBooleanSignal({defaultValue:()=>false});h.update=()=>{set(true);flush();};`,'value()',{initial:'false',desired:'true'}],
  ['array',`import {createControllableArraySignal} from '@solid-primitives/controlled-signal';`,`const [value,set]=createControllableArraySignal<number>({defaultValue:()=>[1]});h.update=()=>{set([1,2]);flush();};`,'value()',{after:'.then(value=>value.length)'}],
  ['set',`import {createControllableSetSignal} from '@solid-primitives/controlled-signal';`,`const [value,set]=createControllableSetSignal<number>({defaultValue:()=>new Set([1])});h.update=()=>{set(new Set([1,2]));flush();};`,'value()',{after:'.then(value=>value.size)'}],
  ['toggle-member',`import {createToggleState} from '@solid-primitives/controlled-signal';`,`const state=createToggleState();h.update=()=>{state.setIsSelected(true);flush();};`,'state.isSelected()',{initial:'false',desired:'true'}],
  ['changed-wrapper',`import {changed} from '@solid-primitives/promise';`,`const [source,set]=createSignal(1);const value=changed(source);h.update=()=>{set(2);flush();};`,'value()',{name:'promise',initial:'false',desired:'true'}],
  ['history-member',`import {createUndoHistory} from '@solid-primitives/history';`,`const [source,set]=createSignal(1);const state=createUndoHistory(()=>{const n=source();return ()=>set(n);});h.update=()=>{set(2);flush();};`,'state.canUndo()',{name:'history',initial:'false',desired:'true'}],
  ['imported-wrapper',`import {read,set} from './shared';`,`h.update=()=>{set(2);flush();};`,'read()',{files:{'shared.ts':`import {createSignal} from 'solid-js';export const [source,set]=createSignal(1);export function read(){return source();}`}}],
  ['lookup-and-this',``,`const [source,set]=createSignal(1);const api={source,get read(){h.values.lookups=(h.values.lookups??0)+1;return function(this:{source:()=>number}){h.values.correctThis=this.source===source;h.values.calls=(h.values.calls??0)+1;return this.source();};}};h.update=()=>{set(2);flush();};`,'api.read()',{}],
  ['two-sources',``,`const [a,setA]=createSignal(1),[b,setB]=createSignal(1);const value=()=>a()+b();h.update=()=>{setA(2);setB(2);flush();};`,'value()',{initial:'2',desired:'4'}],
  ['async-wrapper',`import {read,source,set} from './shared';`,`h.update=()=>{set(2);flush();};`,'read()',{capture:'source()',files:{'shared.ts':`import {createSignal} from 'solid-js';export const [source,set]=createSignal(1);export async function read(){await Promise.resolve();return source();}`}}],
  ['computed-member',``,`const [value,set]=createSignal(1);const api={value};h.update=()=>{set(2);flush();};`,`api['value']()`,{}],
];
for(const [id,imports,setup,expression,options]of setups)for(const bad of [true,false])
  add(id+'-'+(bad?'target':'control'),imports,`${setup}const result=createMemo(()=>{${bad?'':`const captured=${options.capture??expression};`}return Promise.resolve().then(()=>${bad?expression:'captured'})${options.after??''};});`,{...options,role:bad?'target':'control',pair:id});
for(const bad of [true,false])add('external-callback-'+(bad?'target':'control'),`import {consume} from './consumer';`,
  `const [value,set]=createSignal(1);h.update=()=>{set(2);flush();};const result=createMemo(()=>{${bad?'':'const captured=value();'}return consume(${bad?'value':'()=>captured'});});`,
  {role:bad?'target':'control',pair:'external-callback',files:{'consumer.ts':`export function consume(value:()=>number){return Promise.resolve().then(()=>value());}`}});
const setup=`const [source,set]=createSignal(1);h.update=()=>{set(2);flush();};`;
add('wrapped-untrack-control','',`${setup}const value=()=>untrack(source);const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',desired:'1'});
add('imported-untrack-control',`import {read,set} from './shared';`,`h.update=()=>{set(2);flush();};const result=createMemo(()=>Promise.resolve().then(()=>read()));`,
  {role:'control',desired:'1',files:{'shared.ts':`import {createSignal,untrack} from 'solid-js';const [value,set]=createSignal(1);export {set};export function read(){return untrack(value);}`}});
add('constant-wrapper-control','',`${setup}const value=()=>{h.values.debug=source();return 9;};const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',initial:'9',desired:'9'});
add('constant-member-control','',`${setup}const api={source,read(){h.values.debug=this.source();return 9;}};const result=createMemo(()=>Promise.resolve().then(()=>api.read()));`,{role:'control',initial:'9',desired:'9'});
add('ordinary-function-control','',`${setup}const value=()=>9;const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',initial:'9',desired:'9'});
add('invalid-default-typing',`import {createControllableBooleanSignal} from '@solid-primitives/controlled-signal';`,`const [value]=createControllableBooleanSignal({defaultValue:()=>42});const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',expectedTypingCode:2322});
add('missing-package-member-typing',`import {createUndoHistory} from '@solid-primitives/history';`,`const state=createUndoHistory(()=>{});state.missing();const result=createMemo(()=>Promise.resolve().then(()=>state.canUndo()));`,{name:'history',role:'control',expectedTypingCode:2339});
export default cases;
