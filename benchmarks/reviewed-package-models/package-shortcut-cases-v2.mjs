// Six new consumers after the final raw-package browser profile was sealed.
import earlier from './package-shortcut-cases-v1.mjs';
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[...earlier];
const head=`import {createMemo,Loading,flush} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
const setups=[
  ['map-key-iterator','map',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]),read=()=>[...state.keys()].reduce((a,b)=>a+b,0);h.update=()=>{state.set(2,2);flush();};`,'read()','1','3',false],
  ['namespace-static-store','static-store',`import * as stores from '@solid-primitives/static-store';`,`const [state,set]=stores.createStaticStore({n:1}),read=()=>state.n;h.update=()=>{set({n:2});flush();};`,'read()','1','2',false],
  ['media-derived-key','media',`import {createBreakpoints} from '@solid-primitives/media';`,`const state=createBreakpoints({narrow:'500px',wide:'900px'});`,'state.key','wide','narrow',true],
];
for(const [pair,name,imports,setup,expression,initial,desired,media]of setups)for(const bad of [true,false]){
  const packageName='@solid-primitives/'+name,project=retained.find(row=>row.package===packageName).retainedArtifacts.projectDir;
  cases.push({id:'package-shortcut-heldout-'+pair+'-'+(bad?'target':'control'),package:packageName,app:relative(resolve('rust/target/app-import-metric/apps'),project),
    source:`${head}\n${imports}\nfunction App(){${setup}const result=createMemo(()=>{${bad?'':`const captured=${expression};`}return Promise.resolve().then(()=>${bad?expression:'captured'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);if(media)await page.setViewportSize({width:700,height:700});else await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);},
    provenance:{role:bad?'target':'control',expectedIssue:bad,pair,rules:bad?['reactive-read-after-await']:[],codes:[],family:'package-observer-shortcut',artifactOrigin:'retained-published-package',heldOutAfterRawProfile:true}});
}
export default cases;
