// New consumers after the continuation detector froze; labels only score results.
import {read} from './catalog.mjs';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head=`import {createMemo,createSignal,Loading,flush,getOwner,getObserver} from 'solid-js';import {render} from '@solidjs/web';import {consume} from './consumer';const h=(globalThis as any).__experiment;`;
function add(id,{name='controlled-signal',helper,bad=true,role=bad?'target':'control',mode='scalar',initial='1',afterUpdate=bad?initial:'2',desired=role==='control'&&bad?initial:'2',files={},typingCode=null,extra=''}){
  let imports,setup,value,update;
  if(name==='map'){imports=`import {ReactiveMap} from '@solid-primitives/map';`;setup=`const state=new ReactiveMap([[1,1]]);`;value='state.get(1)!';update='state.set(1,2);flush();';}
  else if(name==='set'){imports=`import {ReactiveSet} from '@solid-primitives/set';`;setup='const state=new ReactiveSet<number>();';value='Number(state.has(1))+1';update='state.add(1);flush();';}
  else{imports=`import {createControllableBooleanSignal} from '@solid-primitives/controlled-signal';`;setup='const [value,set]=createControllableBooleanSignal({defaultValue:()=>false});';value='Number(value())+1';update='set(true);flush();';}
  const invoke=mode==='array'?'Promise.all([invoke(read),invoke(read)])':mode==='generator'?'invoke(read).next()':'invoke(read)';
  const callback=mode==='catch'?`async()=>{try{return await ${invoke};}catch(error){h.values.caught=String(error);return 9;}}`:`()=>${invoke}`;
  const output=mode==='array'?'result()[0]+result()[1]':mode==='object'?'result().value':mode==='generator'?'result().value':'result()';
  const source=`${head}${imports}function App(){${setup}${extra}h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return ${value};};h.update=()=>{${update}};const invoke=consume;const result=createMemo(()=>{${bad?'const read=get;':'const captured=get();const read=()=>captured;'}return Promise.resolve().then(${callback});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(${output})}</p></Loading>;}try{h.dispose=render(()=><App/>,document.getElementById('root')!);}catch(error){h.errors.push(String(error));}`;
  const pkg='@solid-primitives/'+name,install=retained.find(row=>row.package===pkg).retainedArtifacts.projectDir;
  cases.push({id:'async-continuation-'+id,package:pkg,install,source,files,artifactOrigin:'retained-published-package-with-authored-consumer-helpers',stages:[{id:'initial',helper,initial,afterUpdate,desired,role,...(typingCode?{typingCode}:{})}]});
}
const bodies=[
  ['await-read',{helper:'export async function consume(read:()=>number){await Promise.resolve();return read();}'}],
  ['two-awaits',{name:'map',helper:'export async function consume(read:()=>number){await Promise.resolve();await Promise.resolve();return read();}'}],
  ['async-arrow',{name:'set',helper:'export const consume=async(read:()=>number)=>{await Promise.resolve();return read();};'}],
  ['awaited-child',{helper:'async function inner(read:()=>number){await Promise.resolve();return read();}export async function consume(read:()=>number){return await inner(read);}'}],
  ['async-method',{name:'map',helper:'const api={async read(value:()=>number){await Promise.resolve();return value();}};export const consume=(read:()=>number)=>api.read(read);'}],
  ['branch',{name:'set',helper:'export async function consume(read:()=>number){await Promise.resolve();if(read())return read();return 0;}'}],
  ['finally-normal',{helper:'export async function consume(read:()=>number){await Promise.resolve();try{return read();}finally{(globalThis as any).__experiment.values.finallyRan=true;}}'}],
  ['cross-file-child',{name:'map',helper:"import {inner} from './inner';export async function consume(read:()=>number){return await inner(read);}",files:{'inner.ts':'export async function inner(read:()=>number){await Promise.resolve();return read();}'}}],
  ['default-parameter',{name:'set',helper:'export async function consume(read:()=>number,scale=1){await Promise.resolve();return read()*scale;}'}],
  ['concurrent-all',{mode:'array',initial:'2',afterUpdate:'2',desired:'4',helper:'export async function consume(read:()=>number){await Promise.resolve();return read();}'}],
  ['adopted-child',{helper:'async function inner(read:()=>number){await Promise.resolve();return read();}export async function consume(read:()=>number){return inner(read);}'}],
  ['nested-reaction',{name:'map',helper:'export async function consume(read:()=>number){return await Promise.resolve().then(()=>read());}'}],
  ['object-result',{name:'set',mode:'object',helper:'export async function consume(read:()=>number){await Promise.resolve();return {value:read()};}'}],
  ['async-generator',{mode:'generator',helper:'export async function* consume(read:()=>number){await Promise.resolve();yield read();}'}],
  ['observation-budget',{name:'map',helper:'export async function consume(read:()=>number){await Promise.resolve();let value=0;for(let n=0;n<70;n++)value=read();return value;}'}],
];
for(const[id,options]of bodies)for(const bad of[true,false])add(id+'-'+(bad?'target':'control'),{...options,bad,...(!bad&&options.mode==='array'?{afterUpdate:'4'}:{})});
add('constant-control',{name:'map',role:'control',initial:'9',afterUpdate:'9',desired:'9',helper:'export async function consume(read:()=>number){await Promise.resolve();read();return 9;}'});
add('finally-constant-control',{name:'set',role:'control',initial:'9',afterUpdate:'9',desired:'9',helper:'export async function consume(read:()=>number){await Promise.resolve();try{return read();}finally{return 9;}}'});
add('explicit-untrack-control',{role:'control',desired:'1',helper:"import {untrack} from 'solid-js';export async function consume(read:()=>number){await Promise.resolve();return untrack(()=>read());}"});
add('finally-throw-control',{name:'map',role:'control',mode:'catch',initial:'9',afterUpdate:'9',desired:'9',helper:'export async function consume(read:()=>number){await Promise.resolve();try{return read();}finally{throw new Error("expected");}}'});
add('invalid-solid-typing',{role:'typing-exclusion',typingCode:2769,extra:'createSignal<boolean>(1);',helper:'export async function consume(read:()=>number){await Promise.resolve();return read();}'});
add('invalid-map-typing',{name:'map',role:'typing-exclusion',typingCode:2345,extra:"state.get('wrong');",helper:'export async function consume(read:()=>number){await Promise.resolve();return read();}'});
export default cases;
