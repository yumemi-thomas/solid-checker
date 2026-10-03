// Authored after the revision detector freeze; labels enter scoring afterward.
import {read} from './catalog.mjs';
const retained=read('rust/target/primitives-checkpoint/run-browser.json').results,cases=[];
const dynamic='export function consume(read:()=>number){return read();}',constant='export function consume(read:()=>number){read();return 9;}',invalid='export function consume(read:()=>number){const wrong:boolean=1;return read();}';
for(const[name,imports,setup]of[
  ['map',`import {ReactiveMap} from '@solid-primitives/map';`,`const state=new ReactiveMap([[1,1]]),get=()=>state.get(1)!;h.update=()=>{state.set(1,2);flush();};`],
  ['set',`import {ReactiveSet} from '@solid-primitives/set';`,`const state=new ReactiveSet<number>(),get=()=>Number(state.has(1))+1;h.update=()=>{state.add(1);flush();};`],
  ['controlled-signal',`import {createControllableBooleanSignal} from '@solid-primitives/controlled-signal';`,`const [value,set]=createControllableBooleanSignal({defaultValue:()=>false}),get=()=>Number(value())+1;h.update=()=>{set(true);flush();};`],
])for(const tracked of[false,true]){
  const pkg='@solid-primitives/'+name,install=retained.find(row=>row.package===pkg).retainedArtifacts.projectDir;
  const source=`import {createMemo,Loading,flush} from 'solid-js';import {render} from '@solidjs/web';import {consume} from './consumer';${imports}const h=(globalThis as any).__experiment;function App(){${setup}const invoke=consume;const result=createMemo(()=>{${tracked?'const captured=invoke(get);':''}return Promise.resolve().then(()=>${tracked?'captured':'invoke(get)'});});return <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}try{h.dispose=render(()=><App/>,document.getElementById('root')!);}catch(error){h.errors.push(String(error));}`;
  const normal=(id,extra={})=>({id,helper:dynamic,initial:'1',afterUpdate:tracked?'2':'1',desired:'2',role:tracked?'control':'target',...extra});
  cases.push({id:name+'-'+(tracked?'tracked':'stale'),package:pkg,install,source,artifactOrigin:'retained-published-package-with-authored-consumer-helpers',stages:[
    normal('initial'),
    {id:'constant-helper',helper:constant,initial:'9',afterUpdate:'9',desired:'9',role:'control'},
    normal('revert-helper'),
    normal('typing-error',{helper:invalid,typingCode:2322,role:'typing-exclusion'}),
    normal('recover-helper'),
    normal('consumer-comment',{mainComment:'\n// next served consumer revision\n'}),
    normal('configuration',{config:{compilerOptions:{target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:false,skipLibCheck:true,allowJs:true},include:['src']}}),
  ]});
}
export default cases;
