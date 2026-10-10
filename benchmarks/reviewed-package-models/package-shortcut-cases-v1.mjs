// Fresh guarded-package cases after sealing the detector; labels are evaluator-only.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head=`import {createMemo,Loading,untrack,flush} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function observe(initial,desired,media=false){return async page=>{
  await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  if(media)await page.setViewportSize({width:700,height:700});else await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));
  await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);
};}
function add(id,imports,body,{name='static-store',files={},initial='1',desired='2',role='target',pair=id,expectedTypingCode=null,media=false,local=false}={}){
  const packageName=local?'study-shortcut-control':'@solid-primitives/'+name,project=local?resolve('rust/target/app-import-metric/apps/package-shortcut-local-controls'):retained.find(row=>row.package===packageName).retainedArtifacts.projectDir;
  cases.push({id:'package-shortcut-'+id,package:packageName,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,flow:observe(initial,desired,media),
    provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'package-observer-shortcut',artifactOrigin:local?'source-authored-counterexample':'retained-published-package'}});
}
const staticImport=`import {createStaticStore} from '@solid-primitives/static-store';`,staticSetup=`const [state,set]=createStaticStore({n:1});h.update=()=>{set('n',2);flush();};`;
const setups=[
  ['static-property',staticImport,staticSetup,'state.n',{}],
  ['static-wrapper',staticImport,staticSetup+`const read=()=>state.n;`,'read()',{}],
  ['static-nested',staticImport,`const [state,set]=createStaticStore({user:{n:1}});h.update=()=>{set('user',{n:2});flush();};`,'state.user.n',{}],
  ['map-iterator',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]);const read=()=>[...state.values()].length;h.update=()=>{state.set(2,2);flush();};`,'read()',{name:'map'}],
  ['map-membership',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]);const read=()=>state.has(2);h.update=()=>{state.set(2,2);flush();};`,'read()',{name:'map',initial:'false',desired:'true'}],
  ['weakmap-membership',`import {ReactiveWeakMap} from '@solid-primitives/map';`,`const state=new ReactiveWeakMap<object,number>(),key={};const read=()=>state.has(key);h.update=()=>{state.set(key,2);flush();};`,'read()',{name:'map',initial:'false',desired:'true'}],
  ['set-size',`import {ReactiveSet} from '@solid-primitives/set';`,`const state=new ReactiveSet([1]);h.update=()=>{state.add(2);flush();};`,'state.size',{name:'set'}],
  ['set-membership',`import {ReactiveSet} from '@solid-primitives/set';`,`const state=new ReactiveSet([1]);const read=()=>state.has(2);h.update=()=>{state.add(2);flush();};`,'read()',{name:'set',initial:'false',desired:'true'}],
  ['trigger-wrapper',`import {createTriggerCache} from '@solid-primitives/trigger';`,`const [track,dirty]=createTriggerCache<number>();let n=1;const read=()=>{track(1);return n;};h.update=()=>{n=2;dirty(1);flush();};`,'read()',{name:'trigger'}],
  ['media-wrapper',`import {createBreakpoints} from '@solid-primitives/media';`,`const state=createBreakpoints({wide:'900px'}),read=()=>state.wide;`,'read()',{name:'media',initial:'true',desired:'false',media:true}],
  ['unknown-argument',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]);h.update=()=>{state.set(2,2);flush();};`,'state.has(2)',{name:'map',initial:'false',desired:'true'}],
  ['external-callback',`import {read,set} from './shared';import {consume} from './consumer';`,`h.update=()=>{set();flush();};`,'consume(read)',{name:'map',capture:'read()',direct:true,files:{'shared.ts':`import {ReactiveMap} from '@solid-primitives/map';const state=new ReactiveMap([[1,1]]);export function read(){return state.size;}export function set(){state.set(2,2);}`,'consumer.ts':`export function consume(read:()=>number){return Promise.resolve().then(()=>read());}`}}],
  ['async-helper',`import {read,readNow,set} from './shared';`,`h.update=()=>{set();flush();};`,'read()',{name:'map',capture:'readNow()',files:{'shared.ts':`import {ReactiveMap} from '@solid-primitives/map';const state=new ReactiveMap([[1,1]]);export function readNow(){return state.size;}export async function read(){await Promise.resolve();return state.size;}export function set(){state.set(2,2);}`}}],
];
for(const [id,imports,setup,expression,options]of setups)for(const bad of [true,false]){
  const captured=options.capture??expression;const returned=options.direct?(bad?expression:'consume(()=>captured)'):`Promise.resolve().then(()=>${bad?expression:'captured'})`;
  add(id+'-'+(bad?'target':'control'),imports,`${setup}const result=createMemo(()=>{${bad?'':`const captured=${captured};`}return ${returned};});`,{...options,role:bad?'target':'control',pair:id});
}
add('map-untrack-control',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]),read=()=>untrack(()=>state.size);h.update=()=>{state.set(2,2);flush();};const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{name:'map',role:'control',desired:'1'});
add('static-untrack-control',staticImport,`${staticSetup}const read=()=>untrack(()=>state.n);const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{role:'control',desired:'1'});
add('map-debug-control',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]),read=()=>{h.values.debug=state.size;return 9;};h.update=()=>{state.set(2,2);flush();};const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{name:'map',role:'control',initial:'9',desired:'9'});
add('static-debug-control',staticImport,`${staticSetup}const read=()=>{h.values.debug=state.n;return 9;};const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{role:'control',initial:'9',desired:'9'});
add('unrelated-signal-control',`import {read} from 'study-shortcut-control';`,`const result=createMemo(()=>Promise.resolve().then(()=>read()));`,{local:true,role:'control',initial:'9',desired:'9'});
add('ordinary-map-control','',`const state=new Map([[1,1]]);h.update=()=>{state.set(2,2);flush();};const result=createMemo(()=>Promise.resolve().then(()=>state.size));`,{name:'map',role:'control',desired:'1'});
add('wrong-setter-typing',staticImport,`const [state,set]=createStaticStore({n:1});set('n','bad');const result=createMemo(()=>Promise.resolve().then(()=>state.n));`,{role:'control',expectedTypingCode:2345});
add('missing-map-member-typing',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]);const result=createMemo(()=>Promise.resolve().then(()=>state.missing));`,{name:'map',role:'control',expectedTypingCode:2339});
export default cases;
