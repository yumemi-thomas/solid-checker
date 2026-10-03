// Fresh APIs and timing paths authored after the revised collector was frozen.
import { relative, resolve } from 'node:path';
import { read } from './catalog.mjs';
const roots=read('rust/target/cross-package-roots/run.json').results, cases=[];
const prelude=`import { createSignal, createMemo, getOwner, onCleanup, runWithOwner, Loading, untrack, flush } from 'solid-js';
import { render } from '@solidjs/web'; const h=(globalThis as any).__experiment; h.values.calls=0; h.values.cleanups=0; h.values.caught=[];`;
const styles=[
  {id:'lodash-reduce',package:'lodash',imports:`import lodash from 'lodash';`,setup:`const dispatch=(fn:()=>number)=>lodash.reduce([1],(sum:number)=>sum+fn(),0);`},
  {id:'rxjs-filter',package:'rxjs',imports:`import {of,filter,delay,firstValueFrom} from 'rxjs';`,setup:`const dispatch=(fn:()=>number)=>of(1).pipe(filter(()=>fn()>0)).subscribe({error:e=>h.values.caught.push(e.message)});`},
  {id:'neverthrow-maperr',package:'neverthrow',imports:`import {err,ResultAsync} from 'neverthrow';`,setup:`const dispatch=(fn:()=>number)=>err(1).mapErr(()=>fn());`},
  {id:'zod-refine',package:'zod',imports:`import {z} from 'zod';`,setup:`const dispatch=(fn:()=>number)=>z.number().superRefine(()=>{fn();}).parse(1);`},
  {id:'valibot-check',package:'valibot',imports:`import * as v from 'valibot';`,setup:`const dispatch=(fn:()=>number)=>v.parse(v.pipe(v.number(),v.check(()=>fn()>0)),1);`},
  {id:'mutation-cache',package:'@tanstack/query-core',imports:`import {MutationCache,QueryClient} from '@tanstack/query-core';`,setup:`const dispatch=(fn:()=>number)=>{const cache=new MutationCache(),client=new QueryClient({mutationCache:cache}),stop=cache.subscribe(()=>fn());try{cache.build(client,{mutationFn:async()=>1});}finally{stop();}return 1;};`},
];
function add(id,style,role,body,flow,expected={},jsx='<p>transfer callback</p>') {
  const project=roots.find(row=>row.package===style.package).retainedArtifacts.projectDir;
  cases.push({id:'transfer-context-'+id+'-'+role,package:style.package,app:relative(resolve('rust/target/app-import-metric/apps'),project),
    source:`${prelude}\n${style.imports}\nfunction App(){${style.setup}\n${body}\nreturn ${jsx};}
h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,flow,
    provenance:{pair:id,role,expectedIssue:role==='target',callbackExpected:true,rules:[],codes:[],...expected}});
}
function observe(desired){return async page=>{
  await page.evaluate(()=>{const h=globalThis.__experiment;h.attempt('invoke',()=>h.run?.());});await page.waitForTimeout(45);
  await page.evaluate(()=>{const h=globalThis.__experiment;h.flush?.();h.dispose();h.disposals++;});await page.waitForTimeout(10);
  await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:String(h.values.calls)};},desired);
};}
for(const style of styles)for(const repetition of ['two-callers','same-path'])for(const bad of [true,false]){
  const body=`const [,set]=createSignal(1);const shared=()=>{h.values.calls++;set(2);return 1;};
    function first(){dispatch(shared);}function second(){dispatch(shared);}
    ${repetition==='two-callers' ? (bad ? "h.attempt('first',first);h.attempt('second',second);" : 'h.run=()=>{first();second();};') :
      (bad ? "h.attempt('loop',()=>{for(let i=0;i<4;i++)try{dispatch(shared);}catch(e){h.values.caught.push(String(e));}});" : 'h.run=()=>{for(let i=0;i<4;i++)dispatch(shared);};')}`;
  add(style.id+'-'+repetition,style,bad?'target':'control',body,observe(repetition==='two-callers'?'2':'4'),
    {family:'fresh-distinct-and-repeated-callers',codes:['REACTIVE_WRITE_IN_OWNED_SCOPE'],expectedFeedbackSites:bad?(repetition==='two-callers'?2:1):0,
      expectedOccurrences:bad?(repetition==='two-callers'?[1,1]:[4]):[]});
}
const floating={id:'floating-middleware',package:'@floating-ui/dom',imports:`import {computePosition} from '@floating-ui/dom';`,setup:''};
for(const bad of [true,false])add('floating-delayed-cleanup',floating,bad?'target':'control',
  `const owner=getOwner(),reference=document.createElement('button'),floating=document.createElement('div');
    const callback=()=>{h.values.calls++;onCleanup(()=>h.values.cleanups++);};
    h.run=()=>computePosition(reference,floating,{middleware:[{name:'probe',fn:()=>{${bad?'callback()':'runWithOwner(owner,callback)'};return {};}}]});`,
  async page=>{await page.evaluate(async()=>{const h=globalThis.__experiment;await h.run();h.dispose();h.disposals++;h.values.behavior={desired:'1',actual:String(h.values.cleanups)};});},
  {family:'package-async-owner-loss',codes:['NO_OWNER_CLEANUP']});
// Correct-looking async pipelines can lose reactive dependencies silently.
for(const [style,pipeline] of [
  [styles[1],read=>`firstValueFrom(of(0).pipe(delay(0),filter(()=>{h.values.calls++;return ${read}>0;}))).then(()=>${read})`],
  [styles[2],read=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>{h.values.calls++;return ${read};}).match(value=>value,()=>0)`],
  [styles[3],read=>`z.number().transform(async()=>{await Promise.resolve();h.values.calls++;return ${read};}).parseAsync(0)`],
])for(const bad of [true,false])add(style.id+'-late-read',style,bad?'target':'control',
  `const [value,set]=createSignal(1);h.update=()=>{set(2);flush();};const result=createMemo(()=>{${bad?'':'const captured=value();'}return ${pipeline(bad?'value()':'captured')};});`,
  async page=>{await page.waitForFunction(()=>document.getElementById('value')?.textContent==='1');await page.evaluate(()=>globalThis.__experiment.update());
    await page.waitForTimeout(60);await page.evaluate(()=>{const h=globalThis.__experiment;h.values.behavior={desired:'2',actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;});},
  {family:'silent-async-dependency-loss',rules:['reactive-read-after-await'],codes:[]},
  `<Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>`);
for(const style of [styles[0],styles[4]])add(style.id+'-untracked-write',style,'control',
  `const [,set]=createSignal(1);const callback=()=>{h.values.calls++;set(2);return 1;};untrack(()=>dispatch(callback));`,observe('1'),
  {family:'explicit-untracked-write'});
add('unused-callback',styles[2],'control',`const callback=()=>{h.values.calls++;onCleanup(()=>{});return 1;};`,observe('0'),
  {family:'unused-callback',callbackExpected:false});
add('false-branch',styles[5],'control',`const callback=()=>{h.values.calls++;onCleanup(()=>{});return 1;};if(false)dispatch(callback);`,observe('0'),
  {family:'unexecuted-branch',callbackExpected:false});
for(const [id,style,body,code] of [
  ['query-missing-member',styles[5],`new QueryClient().missingMember();`,2339],
  ['valibot-wrong-predicate',styles[4],`v.check<number>((value:string)=>value.length>0);`,2345],
])add(id,style,'control',body,observe('0'),{family:'type-exclusion',callbackExpected:false,expectedTypingCode:code});
export default cases;
