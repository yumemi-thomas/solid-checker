// Fresh authored consumers; installed queue implementation and typings are unchanged.
import {read} from './catalog.mjs';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results.find(row=>row.package==='@solid-primitives/queue'),cases=[];
function add(id,{concurrent=false,bad=true,shape='arrow',result='scalar',constant=false,untracked=false,async=false,typing=false}={}){
  const factory=concurrent?'createConcurrentTaskQueue':'createTaskQueue',value=result==='object'?'{value:read()}':'read()',returned=constant?'9':value;
  const helper=`import {untrack} from 'solid-js';const h=(globalThis as any).__experiment;export function makeTask(read:()=>number){return ${async?'async ':''}${shape==='named'?'function work()':'() =>'}{h.values.calls++;${constant?'read();':''}return ${untracked?'untrack(()=>'+returned+')':result==='promise'?'Promise.resolve('+returned+')':returned};};}`;
  const source=`import {createMemo,createSignal,Loading,flush,getOwner,getObserver} from 'solid-js';import {render} from '@solidjs/web';import {${factory}} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=${factory}<${result==='object'?'{value:number}':'number'}>(${concurrent?'1':''});h.values.calls=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};${typing?'queue.enqueue(3);':''}const result=createMemo(()=>{${bad?'const read=get;':'const captured=get();const read=()=>captured;'}return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<${result==='object'?'{value:number}':'number'}>(resolve=>setTimeout(()=>resolve(${result==='object'?'{value:0}':'0'}),1)));return await queue.enqueue(makeTask(read));});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(${result==='object'?'result().value':'result()'})}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);`;
  const initial=constant?'9':'1',afterUpdate=bad?initial:constant?'9':'2',role=typing?'typing-exclusion':bad&&!constant&&!untracked?'target':'control';
  // The invalid call is unreachable at runtime, while its real typing remains checked.
  const safeSource=typing?source.replace('queue.enqueue(3);','if(false)queue.enqueue(3);'):source;
  cases.push({id:'callback-slot-'+id,package:'@solid-primitives/queue',install:retained.retainedArtifacts.projectDir,source:safeSource,artifactOrigin:'retained-published-queue-with-fresh-source-consumer',stages:[{id:'initial',helper,initial,afterUpdate,desired:role==='target'?'2':afterUpdate,role,...(typing?{typingCode:2345}:{})}]});
}
for(const concurrent of [false,true]){
  const prefix=concurrent?'concurrent':'serial';
  for(const [shape,result]of [['arrow','scalar'],['named','scalar'],['arrow','promise'],['arrow','object']])for(const bad of [true,false])add(prefix+'-'+shape+'-'+result+'-'+(bad?'target':'capture'),{concurrent,shape,result,bad});
  add(prefix+'-constant-control',{concurrent,constant:true});add(prefix+'-explicit-untrack-control',{concurrent,untracked:true});add(prefix+'-async-callback-target',{concurrent,async:true});
}
add('published-task-typing-exclusion',{typing:true});
export default cases;
