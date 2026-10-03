// Fresh async callback consumers after the detector freeze; published bytes unchanged.
import {read} from './catalog.mjs';
const installed=read('rust/target/primitives-checkpoint/run-browser.json').results.find(row=>row.package==='@solid-primitives/queue').retainedArtifacts.projectDir,cases=[];
const micro='await Promise.resolve();',timer='await new Promise<void>(resolve=>setTimeout(resolve,1));';
function add(id,{concurrent=false,bad=true,variant='timer',untracked=false,registrationIntent=false,typing=null}={}){
  const object=variant==='object',implicit=variant==='implicit',terminal=variant==='terminal',constant=variant==='constant'||terminal,initial=constant?'9':implicit?'undefined':typing==='task'?'wrong':'1',factory=concurrent?'createConcurrentTaskQueue':'createTaskQueue',resultType=object?'{value:number}':implicit?'void':'number';
  let body=(variant==='microtask'?micro:timer)+'h.values.calls++;';
  body+=variant==='constant'?'read();return 9;':implicit?'read();':terminal?'read();throw new Error("expected");':variant==='object'?'return {value:read()};':variant==='adoption'?'return Promise.resolve(read());':variant==='budget'?'let value=0;for(let i=0;i<70;i++)value=read();return value;':variant==='child'?'return await child(read);':variant==='awaited-value'?'return await Promise.resolve(read());':variant==='finally'?'try{return read();}finally{h.values.finally++;}':untracked?'return untrack(()=>read());':typing==='task'?'read();return "wrong";':'return read();';
  const helper=`import {untrack} from 'solid-js';const h=(globalThis as any).__experiment;async function child(read:()=>number){${timer}return read();}export function makeTask(read:()=>number){return ${variant==='named'?'async function delayedWork()':'async()=>'}{${body}};}`;
  const call='queue.enqueue(makeTask(read))',invoke=registrationIntent?'untrack(()=>'+call+')':call,execution=terminal?'try{return await '+invoke+';}catch{return 9;}':'return await '+invoke+';';
  const source=`import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';import {${factory}} from '@solid-primitives/queue';import {makeTask} from './consumer';const h=(globalThis as any).__experiment;function App(){const [value,set]=createSignal(1);const queue=${factory}<${resultType}>(${concurrent?'1':''});h.values.calls=0;h.values.finally=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};${typing==='solid'?'if(false)createSignal<boolean>(1);':''}const result=createMemo(()=>{${bad?'const read=get;':'const captured=get();const read=()=>captured;'}return Promise.resolve().then(async()=>{void queue.enqueue(()=>new Promise<${resultType}>(resolve=>setTimeout(()=>resolve(${object?'{value:0}':implicit?'undefined':'0'}),1)));${execution}});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(${object?'result().value':'result()'})}</p></Loading>;}h.dispose=render(()=><App/>,document.getElementById('root')!);`;
  const role=typing?'typing-exclusion':bad&&!constant&&!implicit&&!untracked&&!registrationIntent?'target':'control',afterUpdate=bad?initial:constant||implicit?initial:'2';
  cases.push({id:'async-callback-slot-'+id,package:'@solid-primitives/queue',install:installed,source,artifactOrigin:'retained-published-queue-with-fresh-async-consumer',stages:[{id:'initial',helper,initial,afterUpdate,desired:role==='target'?'2':afterUpdate,role,...(typing?{typingCode:typing==='solid'?2769:2345}:{})}]});
}
for(const concurrent of [false,true]){
  const prefix=concurrent?'concurrent':'serial';
  for(const variant of ['microtask','timer','named','child','finally','awaited-value','object','adoption'])for(const bad of [true,false])add(prefix+'-'+variant+'-'+(bad?'target':'capture'),{concurrent,variant,bad});
  for(const variant of ['constant','implicit','terminal','budget'])add(prefix+'-'+variant,{concurrent,variant});
  add(prefix+'-explicit-untrack',{concurrent,untracked:true});add(prefix+'-registration-untrack',{concurrent,registrationIntent:true});
}
add('published-task-typing-exclusion',{typing:'task'});add('published-solid-typing-exclusion',{typing:'solid'});
export default cases;
