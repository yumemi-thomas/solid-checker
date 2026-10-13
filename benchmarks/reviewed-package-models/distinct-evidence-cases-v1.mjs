// Fresh budget cases; installed packages/published typings are unchanged.
import {read} from './catalog.mjs';
const install=read('rust/target/primitives-checkpoint/run-browser.json').results.find(row=>row.package==='@solid-primitives/queue').retainedArtifacts.projectDir,cases=[];
function add(id,{concurrent=false,bad=true,count=70,unique=1,child=false,constant=false,terminal=false,untracked=false}={}){
  const factory=concurrent?'createConcurrentTaskQueue':'createTaskQueue',body=terminal?'throw new Error("expected");':'return '+(constant?'9':'value')+';',helper=`import {untrack} from 'solid-js';async function child(read:()=>number){await Promise.resolve();return read();}export function makeTask(read:()=>number){return async()=>{await Promise.resolve();let value=0;for(let i=0;i<${count};i++)value=${untracked?'untrack(()=>read())':child?'await child(read)':'read()'};${body}};}`;
  const source=`import {createMemo,createSignal,Loading,flush,getOwner,getObserver} from 'solid-js';import {render} from '@solidjs/web';import {${factory}} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const signals=Array.from({length:${unique}},()=>createSignal(1));const queue=${factory}<number>(${concurrent?'1':''});h.values.calls=0;h.values.getterContexts=[];const get=(index:number)=>{h.values.calls++;h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return signals[index][0]();};h.update=()=>{for(const signal of signals)signal[1](2);flush();};const result=createMemo(()=>{let cursor=0;${bad?'const read=()=>get(cursor++%signals.length);':'const captured=signals.map((_,index)=>get(index));const read=()=>captured[cursor++%captured.length];'}return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<number>(resolve=>setTimeout(()=>resolve(0),1)));${terminal?'try{return await queue.enqueue(makeTask(read));}catch{return 9;}':'return await queue.enqueue(makeTask(read));'}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);`;
  const initial=constant||terminal?'9':'1',afterUpdate=bad?initial:constant||terminal?initial:'2',role=bad&&!constant&&!terminal&&!untracked?'target':'control';
  cases.push({id:'distinct-evidence-'+id,package:'@solid-primitives/queue',install,source,artifactOrigin:'retained-published-queue-with-fresh-repeated-or-distinct-native-reads',stages:[{id:'initial',helper,initial,afterUpdate,desired:role==='target'?'2':afterUpdate,role}]});
}
for(const concurrent of [false,true]){
  const prefix=concurrent?'concurrent':'serial';
  for(const count of [30,64,70,200])for(const bad of [true,false])add(prefix+'-repeated-'+count+'-'+(bad?'target':'capture'),{concurrent,count,bad});
  for(const bad of [true,false])add(prefix+'-child-70-'+(bad?'target':'capture'),{concurrent,child:true,bad});
  for(const unique of [64,65])for(const bad of [true,false])add(prefix+'-distinct-'+unique+'-'+(bad?'target':'capture'),{concurrent,count:unique,unique,bad});
  add(prefix+'-constant-control',{concurrent,constant:true});add(prefix+'-terminal-control',{concurrent,terminal:true});add(prefix+'-untrack-control',{concurrent,untracked:true});
}
export default cases;
