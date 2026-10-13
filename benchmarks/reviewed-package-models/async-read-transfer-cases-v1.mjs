// Fresh follow-up after all detector/runtime modules were sealed together.
import {read} from './catalog.mjs';
import {relative,resolve} from 'node:path';
const roots=read('rust/target/cross-package-roots/run.json').results,cases=[];
const head=`import {createSignal,createMemo,Loading,untrack,flush} from 'solid-js';import * as S from 'solid-js';import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function observe(initial,desired){return async page=>{await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  await page.evaluate(()=>{const h=globalThis.__experiment;h.attempt('update',()=>h.update?.());});await page.waitForTimeout(80);
  await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);};}
function add(id,name,imports,body,initial,desired,provenance){const project=roots.find(row=>row.package===name).retainedArtifacts.projectDir;
  cases.push({id:'async-read-transfer-'+id,package:name,app:relative(resolve('rust/target/app-import-metric/apps'),project),
    source:`${head}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{String(result())}</p></Loading>;}
h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,flow:observe(initial,desired),
    provenance:{role:'control',expectedIssue:false,rules:[],codes:[],...provenance}});
}
const entries=[
  ['query-nested','@tanstack/query-core',`import {QueryClient} from '@tanstack/query-core';`,value=>`new QueryClient().fetchQuery({queryKey:['nested'],queryFn:async()=>{await Promise.resolve();return {a:[${value}]};}}).then(value=>value.a[0])`],
  ['floating-data','@floating-ui/dom',`import {computePosition} from '@floating-ui/dom';`,value=>`computePosition(document.createElement('button'),document.createElement('div'),{middleware:[{name:'read',fn:async()=>{await Promise.resolve();return {data:{n:${value}}};}}]}).then(value=>value.middlewareData.read.n)`],
  ['neverthrow-alias','neverthrow',`import {ResultAsync} from 'neverthrow';`,value=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>[{n:${value}}]).match(value=>value[0].n,()=>0)`],
  ['rxjs-named-reader','rxjs',`import {of,delay,map,firstValueFrom} from 'rxjs';`,value=>`(()=>{function read(){return ${value};}return firstValueFrom(of(0).pipe(delay(0),map(read)));})()`],
  ['valibot-cast','valibot',`import * as v from 'valibot';`,value=>`v.parseAsync(v.pipeAsync(v.number(),v.transformAsync(async()=>{await Promise.resolve();return {n:((${value} as number) satisfies number)};})),0).then(value=>value.n)`],
  ['lodash-argument','lodash',`import lodash from 'lodash';`,value=>`Promise.resolve().then(()=>lodash.identity(${value}))`],
  ['mutable-alias','neverthrow',`import {ResultAsync} from 'neverthrow';`,value=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>${value}).match(value=>value,()=>0)`],
  ['promise-argument','neverthrow',`import {ResultAsync} from 'neverthrow';`,value=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>Promise.resolve(${value})).match(value=>value,()=>0)`],
];
for(const [id,name,imports,pipeline]of entries)for(const bad of [true,false]){
  const getter=id==='mutable-alias'?'alias()':id==='neverthrow-alias'?'copy()':'value()';
  const body=`const source=S.createSignal,derive=S.createMemo;const [value,set]=source(1);
    ${id==='mutable-alias'?'let alias=value;':id==='neverthrow-alias'?'const alias=value,copy=alias;':''}
    h.update=()=>{set(2);flush();};const result=derive(()=>{${bad?'':'const captured=value();'}return ${pipeline(bad?getter:'captured')};});`;
  add(id+'-'+(bad?'target':'control'),name,imports,body,'1','2',{pair:id,role:bad?'target':'control',expectedIssue:bad,family:'fresh-async-dependency-loss',rules:['reactive-read-after-await']});
}
const neverthrow=`import {ResultAsync} from 'neverthrow';`,rxjs=`import {of,delay,map,firstValueFrom} from 'rxjs';`;
for(const [id,name,imports,memo,desired]of [
  ['untrack-alias','neverthrow',neverthrow,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>ignore(()=>value())).match(value=>value,()=>0)`,'1'],
  ['unused-result','rxjs',rxjs,`new Promise<number>(resolve=>{setTimeout(()=>{const inspected=(()=>value())();resolve(9);},0);})`,'9'],
  ['discarded-call','rxjs',rxjs,`new Promise<number>(resolve=>{setTimeout(()=>{(()=>value())();resolve(9);},0);})`,'9'],
  ['inspected-result','lodash',`import lodash from 'lodash';`,`new Promise<number>(resolve=>{lodash.defer(()=>{const inspected=lodash.attempt(()=>value());h.values.debug=inspected;resolve(9);});})`,'9'],
  ['unknown-value-use','neverthrow',neverthrow,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>((n:number)=>9)(value())).match(value=>value,()=>0)`,'9'],
  ['discarded-array','rxjs',rxjs,`firstValueFrom(of(0).pipe(delay(0),map(()=>{const inspected=[value()];return 9;})))`,'9'],
  ['captured-before-delivery','neverthrow',neverthrow,`(()=>{const captured=value();return ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>captured).match(value=>value,()=>0);})()`,'2'],
  ['nonreactive-shadow','rxjs',rxjs,`firstValueFrom(of(0).pipe(delay(0),map(()=>fixed())))`,'9'],
  ['untaken-read-branch','neverthrow',neverthrow,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>false?value():9).match(value=>value,()=>0)`,'9'],
])add(id+'-control',name,imports,`const [value,set]=createSignal(1);const ignore=S.untrack;const fixed=()=>9;h.update=()=>{set(2);flush();};const result=createMemo(()=>${memo});`,
  id==='captured-before-delivery'?'1':desired,desired,{family:'fresh-intent-control'});
for(const [id,name,imports,statement,code]of [
  ['query-member','@tanstack/query-core',`import {QueryClient} from '@tanstack/query-core';`,`new QueryClient().missingMember();`,2339],
  ['accessor-argument','neverthrow',neverthrow,`value(42);`,2554],
])add(id+'-typing-exclusion',name,imports,`const [value]=createSignal(1);${statement}const result=createMemo(()=>Promise.resolve().then(()=>value()));`,'1','1',
  {family:'type-exclusion',expectedTypingCode:code});
export default cases;
