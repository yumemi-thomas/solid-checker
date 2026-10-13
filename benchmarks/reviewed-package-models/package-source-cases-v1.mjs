// Installed-source challenges authored after the source bridge was sealed.
// Labels score output only; no package/helper names are detection premises.
import {read} from './catalog.mjs';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const head="import {createMemo,createSignal,Loading,flush,getOwner,getObserver,untrack} from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;";
function add(id,{kind='plain',bad=true,role=bad?'target':'control',mode='scalar',constant=false,namespace=false,delay='1',failures=1,untracked=false,typingCode=null,extra='',queue=false,concurrent=false}={}){
  const pkg=queue?'@solid-primitives/queue':'@solid-primitives/promise',install=retained.find(row=>row.package===pkg).retainedArtifacts.projectDir;
  const imports=queue?`import {${concurrent?'createConcurrentTaskQueue':'createTaskQueue'}} from '${pkg}';`:namespace?`import * as promise from '${pkg}';`:`import {retry} from '${pkg}';`;
  const setup=queue?`const queue=${concurrent?'createConcurrentTaskQueue<number>(1)':'createTaskQueue<number>()'};`:'';
  const returned=constant?'9':mode==='object'?'{value:read()}':'read()';
  const failure=kind==='terminal'?'h.values.attempts++;read();throw new Error("expected");':`h.values.attempts++;if(attempt++<${failures})${kind==='async'?'throw new Error("expected retry");':'return Promise.reject(new Error("expected retry"));'}${constant?'read();':''}return ${kind==='async'?returned:'Promise.resolve('+returned+')'};`;
  let call=queue?`queue.enqueue(()=>{h.values.attempts++;return read();})`:`${namespace?'promise.retry':'retry'}(${kind==='async'?'async ':''}()=>{${failure}},{times:${failures+1},delay:${delay}})`;
  if(queue)call=`async()=>{void queue.enqueue(()=>new Promise<number>(resolve=>setTimeout(()=>resolve(0),1)));return await ${call};}`;
  else if(kind==='terminal')call=`async()=>{try{return await ${call};}catch{return 9;}}`;
  else call=`()=>${untracked?'untrack(()=>'+call+')':call}`;
  const source=`${head}${imports}function App(){const [value,set]=createSignal(1);${setup}${extra}h.values.attempts=0;h.values.getterContexts=[];const get=()=>{h.values.getterContexts.push({owner:!!getOwner(),observer:!!getObserver()});return value();};h.update=()=>{set(2);flush();};const result=createMemo(()=>{${bad?'const read=get;':'const captured=get();const read=()=>captured;'}let attempt=0;return Promise.resolve().then(${call});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(${mode==='object'?'result().value':'result()'})}</p></Loading>;}try{h.dispose=render(()=><App/>,document.getElementById('root')!);}catch(error){h.errors.push(String(error));}`;
  const initial=constant||kind==='terminal'?'9':'1',afterUpdate=bad?initial:constant||kind==='terminal'?'9':'2',desired=role==='target'?'2':afterUpdate;
  cases.push({id:'package-source-'+id,package:pkg,install,source,artifactOrigin:'retained-published-package-and-typings-with-authored-consumer',stages:[{id:'initial',helper:'export const unused=0;',initial,afterUpdate,desired,role,...(typingCode?{typingCode}:{})}]});
}
for(const[id,options]of[
  ['retry-after-timer',{}],
  ['retry-after-rejection',{delay:'0'}],
  ['retry-async-callback',{kind:'async'}],
  ['retry-delay-callback',{delay:'()=>read()'}],
  ['retry-namespace',{namespace:true}],
  ['retry-three-attempts',{failures:2}],
  ['retry-immediate-success',{failures:0}],
  ['retry-object-result',{mode:'object'}],
  ['queue-second-task',{queue:true}],
  ['concurrent-queue-second-task',{queue:true,concurrent:true}],
])for(const bad of[true,false])add(id+'-'+(bad?'target':'control'),{...options,bad});
add('retry-discarded-read-control',{constant:true,role:'control'});
add('retry-async-discarded-read-control',{constant:true,kind:'async',role:'control'});
add('retry-terminal-rejection-control',{kind:'terminal',role:'control'});
add('retry-explicit-untrack-control',{untracked:true,role:'control'});
// TypeScript exclusions use the actual published signatures, unchanged.
add('invalid-solid-typing',{extra:'createSignal<boolean>(1);',role:'typing-exclusion',typingCode:2769});
add('invalid-retry-typing',{extra:'retry(()=>1);',role:'typing-exclusion',typingCode:2322});
export default cases;
