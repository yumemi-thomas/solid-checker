// Fresh consumers authored after the native identity detector freeze.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head=`import {createSignal,createMemo,Loading,flush,untrack} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function observe(initial,desired,viewport=false){return async page=>{
  await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  if(viewport)await page.setViewportSize({width:390,height:844});else await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));
  await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);
};}
function add(id,imports,body,{packageName='@solid-primitives/pagination',files={},initial='1',desired='2',role='target',viewport=false,expectedTypingCode=null,pair=id}={}){
  const project=retained.find(row=>row.package===packageName).retainedArtifacts.projectDir;
  cases.push({id:'native-identity-'+id,package:packageName,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:observe(initial,desired,viewport),provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'native-function-identity'}});
}
const signal=`import {createSignal} from 'solid-js';export const [value,set]=createSignal(1);`;
const configs=[
  ['shared',`import {value,set} from './shared';`,`h.update=()=>{set(2);flush();};`,{files:{'shared.ts':signal}}],
  ['barrel',`import {read,set} from './barrel';`,`const value=read;h.update=()=>{set(2);flush();};`,{files:{'shared.ts':signal,'barrel.ts':`export {value as read,set} from './shared';`}}],
  ['namespace',`import * as shared from './shared';`,`const value=shared.value;h.update=()=>{shared.set(2);flush();};`,{files:{'shared.ts':signal}}],
  ['factory',`import {make} from './factory';`,`const [value,set]=make();h.update=()=>{set(2);flush();};`,{files:{'factory.ts':`import {createSignal} from 'solid-js';export function make(){return createSignal(1);}`}}],
  ['generic-factory',`import {make} from './factory';`,`const [value,set]=make<number>(1);h.update=()=>{set(2);flush();};`,{files:{'factory.ts':`import {createSignal} from 'solid-js';export function make<T>(initial:T){return createSignal(initial);}`}}],
  ['factory-memo',`import {make} from './factory';`,`const [value,set]=make();h.update=()=>{set(2);flush();};`,{files:{'factory.ts':`import {createSignal,createMemo} from 'solid-js';export function make(){const [source,set]=createSignal(1);return [createMemo(()=>source()*2),set] as const;}`},initial:'2',desired:'4'}],
  ['mutable',``,`const [source,set]=createSignal(1);let value=source;h.update=()=>{set(2);flush();};`,{}],
  ['conditional-alias',``,`const [source,set]=createSignal(1);let value:()=>number=()=>9;if(true)value=source;h.update=()=>{set(2);flush();};`,{}],
  ['pagination',`import {createPagination} from '@solid-primitives/pagination';`,`const [,value,set]=createPagination({pages:3});h.update=()=>{set(2);flush();};`,{}],
  ['segment',`import {createSegment} from '@solid-primitives/pagination';`,`const [page,set]=createSignal(1);const value=createSegment([1,2],1,page);h.update=()=>{set(2);flush();};`,{after:'.then(segment=>segment[0])'}],
  ['media',`import {createMediaQuery} from '@solid-primitives/media';`,`const value=createMediaQuery('(min-width: 800px)');`,{packageName:'@solid-primitives/media',initial:'true',desired:'false',viewport:true}],
];
for(const [id,imports,setup,options]of configs)for(const bad of [true,false])
  add(id+'-'+(bad?'target':'control'),imports,`${setup}const result=createMemo(()=>{${bad?'':'const captured=value();'}return Promise.resolve().then(()=>${bad?'value()':'captured'})${options.after??''};});`,{...options,role:bad?'target':'control',pair:id});
const setup=`const [source,set]=createSignal(1);h.update=()=>{set(2);flush();};`;
for(const [id,binding,expression]of [
  ['wrapper',`const value=()=>source();`,'value()'],
  ['bound-clone',`const value=source.bind(null);`,'value()'],
  ['member',`const api={value:source};`,'api.value()'],
  ['unknown-argument',`const value=source;`,'Promise.resolve(value())'],
])add(id+'-target','',`${setup}${binding}const result=createMemo(()=>Promise.resolve().then(()=>${expression}));`);
add('explicit-snapshot-control','',`${setup}const value=source;const result=createMemo(()=>Promise.resolve().then(()=>untrack(()=>value())));`,{role:'control',desired:'1'});
add('fake-mutable-control','',`${setup}let value:()=>number=source;value=()=>9;const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',initial:'9',desired:'9'});
add('synchronous-control','',`${setup}const value=source;const result=createMemo(()=>[0].map(()=>value())[0]);`,{role:'control'});
add('referenced-inspection-control','',`${setup}const value=source;const result=createMemo(()=>Promise.resolve().then(()=>{const inspected=(()=>value())();h.values.debug=inspected;return 9;}));`,{role:'control',initial:'9',desired:'9'});
add('accessor-argument-typing','',`${setup}const value=source;value(42);const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',expectedTypingCode:2554});
add('package-option-typing',`import {createPagination} from '@solid-primitives/pagination';`,`const [,value]=createPagination({pages:'many'});const result=createMemo(()=>Promise.resolve().then(()=>value()));`,{role:'control',expectedTypingCode:2322});
export default cases;
