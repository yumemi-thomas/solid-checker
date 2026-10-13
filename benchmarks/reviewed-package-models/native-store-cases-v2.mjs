// Correct media flow: start at the retained harness viewport, then cross the breakpoint.
// New property/store challenges authored after the store detector was sealed.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head=`import {createSignal,createStore,createMemo,Loading,untrack,flush} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function observe(initial,desired,media=false){return async page=>{
  await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  if(media)await page.setViewportSize({width:700,height:700});
  else await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));
  await page.waitForTimeout(100);
  await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);
};}
function add(id,imports,body,{name='controlled-signal',files={},initial='1',desired='2',role='target',pair=id,expectedTypingCode=null,media=false}={}){
  const packageName='@solid-primitives/'+name,project=retained.find(row=>row.package===packageName).retainedArtifacts.projectDir;
  cases.push({id:'native-store-'+id,package:packageName,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:observe(initial,desired,media),provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'native-store-and-property-read'}});
}
const setups=[
  ['own-property','',`const [state,set]=createStore({n:1});h.update=()=>{set(d=>{d.n=2;});flush();};`,'state.n',{}],
  ['nested-property','',`const [state,set]=createStore({user:{n:1}});h.update=()=>{set(d=>{d.user.n=2;});flush();};`,'state.user.n',{}],
  ['array-length','',`const [state,set]=createStore({list:[1]});h.update=()=>{set(d=>{d.list.push(2);});flush();};`,'state.list.length',{}],
  ['materialized-property','',`const [state,set]=createStore({n:1});const tracked=createMemo(()=>state.n);h.values.materialized=tracked();h.update=()=>{set(d=>{d.n=2;});flush();};`,'state.n',{}],
  ['imported-store',`import {state,set} from './shared';`,`h.update=()=>{set(d=>{d.n=2;});flush();};`,'state.n',{files:{'shared.ts':`import {createStore} from 'solid-js';export const [state,set]=createStore({n:1});`}}],
  ['accessor-property','',`const [state,set]=createStore({n:1,get doubled(){return this.n*2;}});h.update=()=>{set(d=>{d.n=2;});flush();};`,'state.doubled',{initial:'2',desired:'4'}],
  ['history-wrapper',`import {createUndoHistory} from '@solid-primitives/history';`,`const [source,set]=createSignal(1);const state=createUndoHistory(()=>{const n=source();return ()=>set(n);});const read=()=>state.canUndo();h.update=()=>{set(2);flush();};`,'read()',{name:'history',initial:'false',desired:'true'}],
  ['map-size',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]);h.update=()=>{state.set(2,2);flush();};`,'state.size',{name:'map'}],
  ['media-field',`import {createBreakpoints} from '@solid-primitives/media';`,`const state=createBreakpoints({wide:'900px'});`,'state.wide',{name:'media',initial:'true',desired:'false',media:true}],
  ['absent-property','',`const [state,set]=createStore<{n?:number}>({});h.update=()=>{set(d=>{d.n=2;});flush();};`,'state.n??0',{initial:'0'}],
  ['computed-property','',`const [state,set]=createStore({n:1});h.update=()=>{set(d=>{d.n=2;});flush();};`,"state['n']",{}],
  ['optional-property','',`const [state,set]=createStore({n:1});h.update=()=>{set(d=>{d.n=2;});flush();};`,'state?.n',{}],
  ['then-property','',`const [state,set]=createStore({then:1});h.update=()=>{set(d=>{d.then=2;});flush();};`,'state.then',{}],
  ['async-store-wrapper',`import {read,state,set} from './shared';`,`h.update=()=>{set(d=>{d.n=2;});flush();};`,'read()',{capture:'state.n',files:{'shared.ts':`import {createStore} from 'solid-js';export const [state,set]=createStore({n:1});export async function read(){await Promise.resolve();return state.n;}`}}],
];
for(const [id,imports,setup,expression,options]of setups)for(const bad of [true,false])
  add(id+'-'+(bad?'target':'control'),imports,`${setup}const result=createMemo(()=>{${bad?'':`const captured=${options.capture??expression};`}return Promise.resolve().then(()=>${bad?expression:'captured'});});`,{...options,role:bad?'target':'control',pair:id});
for(const bad of [true,false])add('external-store-callback-'+(bad?'target':'control'),`import {consume} from './consumer';`,
  `const [state,set]=createStore({n:1});h.update=()=>{set(d=>{d.n=2;});flush();};const result=createMemo(()=>{${bad?'':'const captured=state.n;'}return consume(${bad?'()=>state.n':'()=>captured'});});`,
  {role:bad?'target':'control',pair:'external-store-callback',files:{'consumer.ts':`export function consume(value:()=>number){return Promise.resolve().then(()=>value());}`}});
const setup=`const [state,set]=createStore({n:1});h.update=()=>{set(d=>{d.n=2;});flush();};`;
add('explicit-untrack-control','',`${setup}const result=createMemo(()=>Promise.resolve().then(()=>untrack(()=>state.n)));`,{role:'control',desired:'1'});
add('wrapped-untrack-control','',`${setup}const read=()=>untrack(()=>state.n);const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{role:'control',desired:'1'});
add('constant-debug-control','',`${setup}const api={read(){h.values.debug=state.n;return 9;}};const result=createMemo(()=>Promise.resolve().then(()=>api.read()));`,{role:'control',initial:'9',desired:'9'});
add('ordinary-property-control','',`const state={n:9};const result=createMemo(()=>Promise.resolve().then(()=>state.n));`,{role:'control',initial:'9',desired:'9'});
add('ordinary-proxy-control','',`const state=new Proxy({n:9},{get(target,key,receiver){h.values.lookups=(h.values.lookups??0)+1;return Reflect.get(target,key,receiver);}});const result=createMemo(()=>Promise.resolve().then(()=>state.n));`,{role:'control',initial:'9',desired:'9'});
add('store-snapshot-control','',`${setup}const result=createMemo(()=>Promise.resolve().then(()=>{h.values.debug=state.n;return 9;}));`,{role:'control',initial:'9',desired:'9'});
add('missing-property-typing','',`${setup}const result=createMemo(()=>Promise.resolve().then(()=>state.missing));`,{role:'control',expectedTypingCode:2339});
add('wrong-setter-value-typing','',`const [state,set]=createStore({n:1});set(d=>{d.n='bad';});const result=createMemo(()=>Promise.resolve().then(()=>state.n));`,{role:'control',expectedTypingCode:2322});
export default cases;

