// Fresh value-flow counterexamples after the precision projector was sealed.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const cases=[],retained=read('rust/target/primitives-checkpoint/run-browser.json').results;
const head=`import {createMemo,createSignal,Loading,flush} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function add(id,{name='map',imports='',setup='',expression,bad=true,role=bad?'target':'control',pair=id,initial='1',desired='2',files={},expectedTypingCode=null}){
  const pkg='@solid-primitives/'+name,project=retained.find(row=>row.package===pkg).retainedArtifacts.projectDir;
  cases.push({id:'constant-result-'+id,package:pkg,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${setup}const result=createMemo(()=>{${bad?'':`const captured=${expression};`}return Promise.resolve().then(()=>${bad?expression:'captured'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);},
    provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'constant-result-precision',artifactOrigin:'retained-published-package-with-authored-consumer-helpers',heldOutAfterConstantProjector:true}});
}
const mapImport=`import {ReactiveMap} from '@solid-primitives/map';`,mapSetup=`const state=new ReactiveMap([[1,1]]);h.update=()=>{state.set(1,2);flush();};`;
const registration={name:'trigger',imports:`import {createTriggerCache} from '@solid-primitives/trigger';`,setup:`const [track,dirty]=createTriggerCache<number>();let raw=1;const register=()=>{track(1);return 9;};h.update=()=>{raw=2;dirty(1);flush();};`};
const pairs=[
  ['registration-binary',{...registration,expression:'register()+raw',initial:'10',desired:'11'}],
  ['registration-wrapper',{...registration,setup:registration.setup+`const read=()=>{register();return raw;};`,expression:'read()'}],
  ['registration-condition',{...registration,expression:'register()?raw:raw'}],
  ['mixed-literals',{imports:mapImport,setup:`const state=new ReactiveMap([[1,1]]);const read=()=>{if(state.has(1))return 1;return 2;};h.update=()=>{state.delete(1);flush();};`,expression:'read()'}],
  ['object-result',{imports:mapImport,setup:mapSetup+`const read=()=>({n:state.get(1)});`,expression:'read().n'}],
  ['array-result',{imports:mapImport,setup:mapSetup+`const read=()=>[state.get(1)];`,expression:'read()'}],
  ['mutable-member',{imports:mapImport,setup:mapSetup+`const api={read:()=>9};api.read=()=>state.get(1)!;`,expression:'api.read()'}],
  ['mutable-export-js',{imports:mapImport+`import {read,replace} from './consumer';`,setup:mapSetup+`replace(()=>state.get(1));`,expression:'read()',files:{'consumer.js':`export function read(){return 9;}export function replace(fn){read=fn;}`}}],
  ['destructured-export-js',{imports:mapImport+`import {read,replace} from './consumer';`,setup:mapSetup+`replace(()=>state.get(1));`,expression:'read()',files:{'consumer.js':`export function read(){return 9;}export function replace(fn){({read}={read:fn});}`}}],
  ['mutable-local',{imports:mapImport,setup:mapSetup+`let read=()=>9;read=()=>state.get(1)!;`,expression:'read()'}],
  ['shadowed-inner',{imports:mapImport,setup:mapSetup+`const read=()=>{state.get(1);return 9;};`,expression:'(()=>{const read=()=>state.get(1);return read();})()'}],
];
for(const[pair,options]of pairs)for(const bad of[true,false])add(pair+'-'+(bad?'target':'control'),{...options,bad,pair});
const control=(id,options)=>add(id+'-control',{...options,bad:true,role:'control',initial:options.initial??'9',desired:options.desired??'9'});
control('constant-alias',{imports:mapImport,setup:mapSetup+`const original=()=>{state.get(1);return 9;};const read=(original satisfies ()=>number);`,expression:'read()'});
control('ignored-scalar',{imports:mapImport,setup:mapSetup+`const consume=(_value:number)=>9;`,expression:'(consume satisfies (value:number)=>number)(state.get(1)!)'});
control('same-branches',{imports:mapImport,setup:mapSetup+`function read(){if(state.get(1))return 9;return 9;}`,expression:'read()'});
const shared={'shared.ts':`import {ReactiveMap} from '@solid-primitives/map';const state=new ReactiveMap([[1,1]]);export function read(){return state.get(1)!;}export function set(){state.set(1,2);}`,'consumer.ts':`export function consume(read:()=>number){read();return 9;}`};
control('named-import',{imports:`import {read,set} from './shared';import {consume} from './consumer';`,setup:`h.update=()=>{set();flush();};`,expression:'consume(read)',files:shared});
control('namespace-import',{imports:`import {read,set} from './shared';import * as helpers from './consumer';`,setup:`h.update=()=>{set();flush();};`,expression:'helpers.consume(read)',files:shared});
control('constant-null',{imports:mapImport,setup:mapSetup+`const read=()=>{state.get(1);return null;};`,expression:'read()',initial:'null',desired:'null'});
control('constant-boolean',{imports:mapImport,setup:mapSetup+`const read=()=>{state.get(1);return false;};`,expression:'read()',initial:'false',desired:'false'});
control('constant-registration',{...registration,expression:'register()'});
control('async-constant',{imports:mapImport,setup:mapSetup+`const read=async()=>{state.get(1);return 9;};`,expression:'read()'});
add('invalid-solid-typing',{imports:mapImport,setup:mapSetup+`createSignal<boolean>(1);`,expression:'state.get(1)',role:'control',expectedTypingCode:2769});
add('function-write-typing',{imports:mapImport,setup:mapSetup+`function read(){return 9;}read=()=>state.get(1)!;`,expression:'read()',role:'control',expectedTypingCode:2630});
add('map-key-typing',{imports:mapImport,setup:mapSetup,expression:"state.get('bad')",role:'control',expectedTypingCode:2345});
export default cases;
