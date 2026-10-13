// Consumers authored after the argument detector and direct-eval boundary froze.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const cases=[],retained=read('rust/target/primitives-checkpoint/run-browser.json').results;
const head=`import {createMemo,Loading,flush,untrack} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function add(id,{name='map',imports='',setup='',expression,capture=expression,bad=true,role=bad?'target':'control',pair=id,initial='1',desired='2',files={},expectedTypingCode=null,local=false}){
  const pkg=local?'study-accessor-use':'@solid-primitives/'+name,project=local?resolve('rust/target/app-import-metric/apps/accessor-use-local-challenge'):retained.find(row=>row.package===pkg).retainedArtifacts.projectDir;
  cases.push({id:'argument-read-'+id,package:pkg,app:relative(resolve('rust/target/app-import-metric/apps'),project),files,
    source:`${head}\n${imports}\nfunction App(){${setup}const result=createMemo(()=>{${bad?'':`const captured=${capture};`}return Promise.resolve().then(async()=>${bad?expression:'captured'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,
    flow:async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);await page.evaluate(()=>globalThis.__experiment.attempt('update',()=>globalThis.__experiment.update?.()));await page.waitForTimeout(100);await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);},
    provenance:{role,expectedIssue:role==='target',pair,rules:role==='target'?['reactive-read-after-await']:[],codes:[],...(expectedTypingCode?{expectedTypingCode}:{}),family:'argument-read',artifactOrigin:local?'source-authored-package':'retained-published-package',heldOutAfterArgumentProfile:true}});
}
const mapImport=`import {ReactiveMap,ReactiveWeakMap} from '@solid-primitives/map';`,mapSetup=`const state=new ReactiveMap([[1,1]]);h.update=()=>{state.set(1,2);flush();};`;
const setImport=`import {ReactiveSet,ReactiveWeakSet} from '@solid-primitives/set';`;
const staticFiles={
  'shared.ts':`import {createStaticStore} from '@solid-primitives/static-store';const [state,setState]=createStaticStore({n:1});export function read(){return state.n;}export function set(){setState('n',2);}`,
  'consumer.ts':`export function consume(read:()=>number){return read();}`,
};
const setups=[
  ['map-value',{imports:mapImport,setup:mapSetup,expression:'state.get(1)'}],
  ['map-presence',{imports:mapImport,setup:`const state=new ReactiveMap([[1,1]]);h.update=()=>{state.set(2,2);flush();};`,expression:'state.has(2)',initial:'false',desired:'true'}],
  ['weakmap-value',{imports:mapImport,setup:`const key={},state=new ReactiveWeakMap<object,number>([[key,1]]);h.update=()=>{state.set(key,2);flush();};`,expression:'state.get(key)'}],
  ['weakmap-presence',{imports:mapImport,setup:`const key={},state=new ReactiveWeakMap<object,number>();h.update=()=>{state.set(key,2);flush();};`,expression:'state.has(key)',initial:'false',desired:'true'}],
  ['set-presence',{name:'set',imports:setImport,setup:`const state=new ReactiveSet([1]);h.update=()=>{state.add(2);flush();};`,expression:'state.has(2)',initial:'false',desired:'true'}],
  ['weakset-presence',{name:'set',imports:setImport,setup:`const key={},state=new ReactiveWeakSet<object>();h.update=()=>{state.add(key);flush();};`,expression:'state.has(key)',initial:'false',desired:'true'}],
  ['spread-key',{imports:mapImport,setup:mapSetup+`const keys:[number]=[1];`,expression:'state.get(...keys)'}],
  ['function-call',{imports:mapImport,setup:mapSetup,expression:'state.get.call(state,1)'}],
  ['reflect-apply',{imports:mapImport,setup:mapSetup,expression:'Reflect.apply(state.get,state,[1])'}],
  ['generic-helper',{imports:mapImport,setup:mapSetup+`function lookup<K,V>(map:ReactiveMap<K,V>,key:K){return map.get(key);}`,expression:'lookup(state,1)'}],
  ['synchronous-imported-callback',{name:'static-store',imports:`import {read,set} from './shared';import {consume} from './consumer';`,setup:`h.update=()=>{set();flush();};`,expression:'consume(read)',capture:'read()',files:staticFiles}],
  ['deferred-imported-callback',{name:'static-store',imports:`import {read,set} from './shared';import {consume} from './consumer';`,setup:`h.update=()=>{set();flush();};`,expression:'consume(read)',capture:'read()',files:{...staticFiles,'consumer.ts':`export function consume(read:()=>number){return Promise.resolve().then(read);}`}}],
  ['await-argument',{imports:mapImport,setup:mapSetup,expression:'state.get(await Promise.resolve(1))',capture:'state.get(1)'}],
  ['computed-argument',{imports:mapImport,setup:mapSetup,expression:"state['get'](1)"}],
  ['optional-argument',{imports:mapImport,setup:mapSetup,expression:'state?.get(1)'}],
  ['source-package-argument',{local:true,imports:`import * as api from 'study-accessor-use';`,setup:`h.update=()=>{api.set(2);flush();};`,expression:'api.argument(1)',initial:'2',desired:'3'}],
];
for(const[pair,options]of setups)for(const bad of[true,false])add(pair+'-'+(bad?'target':'control'),{...options,bad,pair});
const control=(id,options)=>add(id+'-control',{...options,bad:true,role:'control',initial:'9',desired:'9'});
control('ordinary-helper',{setup:`const consume=(value:number)=>value+8;`,expression:'consume(1)'});
add('ordinary-map-control',{setup:`const state=new Map([[1,1]]);h.update=()=>{state.set(1,2);flush();};`,expression:'state.get(1)',role:'control',desired:'1'});
control('ignored-scalar',{imports:mapImport,setup:mapSetup+`const consume=(_value:number)=>9;`,expression:'consume(state.get(1)!)'});
control('discarded-callback-read',{name:'static-store',imports:`import {read,set} from './shared';import {consume} from './consumer';`,setup:`h.update=()=>{set();flush();};`,expression:'consume(read)',files:{...staticFiles,'consumer.ts':`export function consume(read:()=>number){read();return 9;}`}});
control('throwing-argument',{imports:mapImport,setup:mapSetup+`const consume=(_value:number)=>9;const fail=():never=>{throw Error('argument failed');};`,expression:`(()=>{try{return consume((state.get(1),fail()));}catch(error){h.values.thrown=(error as Error).message;return 9;}})()`});
control('throwing-spread',{imports:mapImport,setup:mapSetup+`const consume=(...values:number[])=>9;const values={*[Symbol.iterator](){yield state.get(1)!;throw Error('spread failed');}};`,expression:`(()=>{try{return consume(...values);}catch(error){h.values.thrown=(error as Error).message;return 9;}})()`});
add('explicit-untrack-control',{imports:mapImport,setup:mapSetup,expression:'untrack(()=>state.get(1))',role:'control',desired:'1'});
control('direct-eval',{expression:"eval('9')"});
add('missing-map-argument-typing',{imports:mapImport,setup:mapSetup,expression:'state.get()',role:'control',expectedTypingCode:2554});
add('wrong-map-argument-typing',{imports:mapImport,setup:mapSetup,expression:"state.get('bad')",role:'control',expectedTypingCode:2345});
add('wrong-weakset-argument-typing',{name:'set',imports:setImport,setup:'const state=new ReactiveWeakSet<object>();',expression:'state.has(1)',role:'control',expectedTypingCode:2345});
export default cases;
