// Native-reader addendum authored after the unchanged argument profile froze.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const cases=[],pkg='@solid-primitives/controlled-signal',project=read('rust/target/primitives-checkpoint/run-browser.json').results.find(row=>row.package===pkg).retainedArtifacts.projectDir;
const head=`import {createMemo,createSignal,Loading,flush} from 'solid-js';import {render} from '@solidjs/web';import {createControllableBooleanSignal,createControllableSignal} from '@solid-primitives/controlled-signal';const h=(globalThis as any).__experiment;`;
function add(id,setup,expression,{bad=true,role=bad?'target':'control',initial='1',desired='2',pair=id}={}){
  cases.push({id:'argument-native-'+id,package:pkg,app:relative(resolve('rust/target/app-import-metric/apps'),project),
    source:`${head}\nfunction App(){${setup}const result=createMemo(()=>{${bad?'':`const captured=${expression};`}return Promise.resolve().then(()=>${bad?expression:'captured'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);},
    provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],family:'argument-native-read',artifactOrigin:'retained-published-package',heldOutAfterArgumentProfile:true}});
}
const boolean=`const [value,set]=createControllableBooleanSignal({defaultValue:()=>false}),read=(offset:number)=>Number(value())+offset;h.update=()=>{set(true);flush();};`;
const controlled=`const [source,setSource]=createSignal(1,{ownedWrite:true});const [value]=createControllableSignal<number>({value:source}),read=(offset:number)=>value()!+offset;h.update=()=>{setSource(2);flush();};`;
for(const bad of[true,false]){
  add('boolean-'+(bad?'target':'control'),boolean,'read(1)',{bad,pair:'boolean'});
  add('controlled-'+(bad?'target':'control'),controlled,'read(1)',{bad,pair:'controlled',initial:'2',desired:'3'});
}
add('ignored-native-scalar-control',boolean+`const consume=(_value:boolean)=>9;`,'consume(value())',{role:'control',initial:'9',desired:'9'});
export default cases;
