// Fresh returned pipelines and intent controls authored after the detector seal.
import {relative,resolve} from 'node:path';
import {read} from './catalog.mjs';
const roots=read('rust/target/cross-package-roots/run.json').results,cases=[];
const prelude=`import {createSignal,createMemo,Loading,flush,untrack} from 'solid-js';import * as S from 'solid-js';
import {render} from '@solidjs/web';const h=(globalThis as any).__experiment;`;
function add(id,name,imports,body,jsx,flow,provenance){const project=roots.find(row=>row.package===name).retainedArtifacts.projectDir;
  cases.push({id:'async-read-'+id,package:name,app:relative(resolve('rust/target/app-import-metric/apps'),project),
    source:`${prelude}\n${imports}\nfunction App(){${body}\nreturn <Loading fallback={<p>waiting</p>}><p id='value'>{${jsx}}</p></Loading>;}
h.attempt('mount',()=>h.dispose=render(()=><App/>,document.getElementById('root')!));`,flow,
    provenance:{rules:[],codes:[],...provenance}});
}
function observe(initial,desired){return async page=>{
  await page.waitForFunction(initial=>document.getElementById('value')?.textContent===initial,initial);
  await page.evaluate(()=>{const h=globalThis.__experiment;h.attempt('update',()=>h.update?.());});await page.waitForTimeout(80);
  await page.evaluate(desired=>{const h=globalThis.__experiment;h.values.behavior={desired,actual:document.getElementById('value')?.textContent};h.dispose();h.disposals++;},desired);
};}
const recipes=[
  ['rxjs-arithmetic','rxjs',`import {of,delay,map,firstValueFrom} from 'rxjs';`,read=>`firstValueFrom(of(0).pipe(delay(0),map(n=>${read}+n)))`,'String(result())'],
  ['neverthrow-arithmetic','neverthrow',`import {ResultAsync} from 'neverthrow';`,read=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(n=>${read}+n).match(n=>n,()=>0)`,'String(result())'],
  ['zod-template','zod',`import {z} from 'zod';`,read=>`z.number().transform(async()=>{await Promise.resolve();return \`\${${read}}\`;}).parseAsync(0)`,'String(result())'],
  ['valibot-async','valibot',`import * as v from 'valibot';`,read=>`v.parseAsync(v.pipeAsync(v.number(),v.transformAsync(async n=>{await Promise.resolve();return ${read}+n;})),0)`,'String(result())'],
  ['query-fetch','@tanstack/query-core',`import {QueryClient} from '@tanstack/query-core';`,read=>`new QueryClient().fetchQuery({queryKey:['read'],queryFn:async()=>{await Promise.resolve();return ${read};}})`,'String(result())'],
  ['floating-structured','@floating-ui/dom',`import {computePosition} from '@floating-ui/dom';`,read=>`computePosition(document.createElement('button'),document.createElement('div'),{middleware:[{name:'read',fn:async()=>{await Promise.resolve();return {x:${read}};}}]}).then(result=>result.x)`,'String(result())'],
  ['neverthrow-object','neverthrow',`import {ResultAsync} from 'neverthrow';`,read=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>({n:${read}})).match(value=>value.n,()=>0)`,'String(result())'],
  ['rxjs-array','rxjs',`import {of,delay,map,firstValueFrom} from 'rxjs';`,read=>`firstValueFrom(of(0).pipe(delay(0),map(()=>[${read}]))).then(value=>value[0])`,'String(result())'],
  ['lodash-resolver','lodash',`import lodash from 'lodash';`,read=>`new Promise<number>(resolve=>{lodash.defer(()=>{resolve(${read});});})`,'String(result())'],
  ['accessor-alias','neverthrow',`import {ResultAsync} from 'neverthrow';`,read=>`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>${read}).match(value=>value,()=>0)`,'String(result())'],
  ['local-reader','rxjs',`import {of,delay,map,firstValueFrom} from 'rxjs';`,read=>`(()=>{const read=()=>${read};return firstValueFrom(of(0).pipe(delay(0),map(read)));})()`,'String(result())'],
];
for(const [id,name,imports,pipeline,jsx]of recipes)for(const bad of [true,false]){
  const body=`const [value,set]=createSignal(1);${id==='accessor-alias'?'const alias=value;':''}h.update=()=>{set(2);flush();};
    const result=createMemo(()=>{${bad?'':'const captured=value();'}return ${pipeline(bad?(id==='accessor-alias'?'alias()':'value()'):'captured')};});`;
  add(id+'-'+(bad?'target':'control'),name,imports,body,jsx,observe('1','2'),
    {pair:id,role:bad?'target':'control',expectedIssue:bad,family:'async-dependency-loss',rules:['reactive-read-after-await']});
}
const resultAsync=`import {ResultAsync} from 'neverthrow';`,rxjs=`import {of,delay,map,firstValueFrom} from 'rxjs';`;
for(const [id,name,imports,memo,desired]of [
  ['explicit-untrack','neverthrow',resultAsync,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>untrack(()=>value())).match(n=>n,()=>0)`,'1'],
  ['namespace-untrack','neverthrow',resultAsync,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>S.untrack(()=>value())).match(n=>n,()=>0)`,'1'],
  ['discarded-read','rxjs',rxjs,`firstValueFrom(of(0).pipe(delay(0),map(()=>{value();return 9;})))`,'9'],
  ['debug-assignment','rxjs',rxjs,`firstValueFrom(of(0).pipe(delay(0),map(()=>{h.values.debug=value();return 9;})))`,'9'],
  ['unknown-argument','neverthrow',resultAsync,`ResultAsync.fromSafePromise(Promise.resolve(0)).map(()=>((n:number)=>9)(value())).match(n=>n,()=>0)`,'9'],
  ['discarded-return','lodash',`import lodash from 'lodash';`,`new Promise<number>(resolve=>{lodash.defer(()=>{const inspected=lodash.attempt(()=>value());resolve(9);});})`,'9'],
  ['synchronous-callback','lodash',`import lodash from 'lodash';`,`Promise.resolve(lodash.reduce([0],n=>n+value(),0))`,'2'],
])add(id+'-control',name,imports,`const [value,set]=createSignal(1);h.update=()=>{set(2);flush();};const result=createMemo(()=>${memo});`,'String(result())',
  observe(id==='synchronous-callback'?'1':desired,desired),{role:'control',expectedIssue:false,family:'read-intent-control'});
for(const [id,name,imports,statement,code]of [
  ['const-reassignment','neverthrow',resultAsync,`value=()=>2;`,2588],
  ['query-invalid-key','@tanstack/query-core',`import {QueryClient} from '@tanstack/query-core';`,`new QueryClient().setQueryData(42,1);`,2345],
  ['rxjs-missing-member','rxjs',rxjs,`of(1).missingMember();`,2339],
])add(id+'-typing-exclusion',name,imports,`const [value]=createSignal(1);${statement}const result=createMemo(()=>Promise.resolve().then(()=>value()));`,'String(result())',
  observe('1','1'),{role:'control',expectedIssue:false,family:'type-exclusion',expectedTypingCode:code});
export default cases;
