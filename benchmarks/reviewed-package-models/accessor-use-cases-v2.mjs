// Real Solid overload error recorded from preflight; async setup has no extra helper.
// Consumers of a source-authored package created after the detector was sealed.
import {relative,resolve} from 'node:path';
const cases=[],project=resolve('rust/target/app-import-metric/apps/accessor-use-local-challenge');
const head=`import {createMemo,Loading,flush,untrack,createSignal} from 'solid-js';import {render} from '@solidjs/web';import * as api from 'study-accessor-use';const h=(globalThis as any).__experiment;`;
function add(id,setup,expression,{role='target',pair=id,initial='1',desired='2',capture=expression,files={},expectedTypingCode=null}={}){
  cases.push({id:'accessor-use-'+id,package:'study-accessor-use',app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\nfunction App(){h.update=()=>{api.set(2);flush();};${setup}const result=createMemo(()=>{${role==='target'?'':`const captured=${capture};`}return Promise.resolve().then(()=>${role==='target'?expression:'captured'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);},
    provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'signal-accessor-use',artifactOrigin:'source-authored-package',heldOutAfterAccessorProfile:true}});
}
const setups=[
  ['direct','','api.direct()',{}],
  ['aliased-getter','','api.aliased()',{}],
  ['tuple','','api.tuple()',{}],
  ['tuple-alias','','api.tupleAlias()',{}],
  ['inline-getter','','api.inline()',{}],
  ['object-lookup','','api.objectLookup()',{}],
  ['argument-wrapper','const read=()=>api.argument(1);','read()',{initial:'2',desired:'3'}],
  ['argument-direct','','api.argument(1)',{initial:'2',desired:'3'}],
  ['computed-member','',"api['direct']()",{}],
  ['async-helper','','read()',{imports:true,capture:'api.aliased()',files:{'shared.ts':`import {aliased} from 'study-accessor-use';export async function read(){await Promise.resolve();return aliased();}`}}],
];
for(const [pair,setup,expression,options]of setups)for(const bad of [true,false]){
  add(pair+'-'+(bad?'target':'control'),setup,expression,{...options,role:bad?'target':'control',pair});
  if(options.imports)cases.at(-1).source=`import {read} from './shared';\n`+cases.at(-1).source;
}
for(const method of ['unused','setterOnly','rebound','escapedUnused','discardedRead']){
  // Control role means an intentionally constant result, so exercise the late read.
  add(method+'-control','',`api.${method}()`,{role:'target',initial:'9',desired:'9'});const row=cases.at(-1);row.provenance={...row.provenance,role:'control',expectedIssue:false,rules:[]};
}
add('wrapped-untrack-control','const read=()=>untrack(api.direct);','read()',{role:'target',desired:'1'});cases.at(-1).provenance={...cases.at(-1).provenance,role:'control',expectedIssue:false,rules:[]};
add('ordinary-function-control','const read=()=>9;','read()',{role:'target',initial:'9',desired:'9'});cases.at(-1).provenance={...cases.at(-1).provenance,role:'control',expectedIssue:false,rules:[]};
add('invalid-solid-typing','createSignal<boolean>(1);','api.direct()',{role:'control',expectedTypingCode:2769});
add('missing-local-member-typing','','api.missing()',{role:'control',expectedTypingCode:2339});
export default cases;

