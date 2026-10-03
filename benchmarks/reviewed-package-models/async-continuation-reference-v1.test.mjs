import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {projectReadSession} from './project-read-session-v2.mjs';
import {transformAsyncContinuations} from './async-continuation-transform-v2.mjs';
import {nativeReads} from './native-read-runtime-v5.mjs';
import {ts} from './lower.mjs';
async function input(code){
  const root=resolve(mkdtempSync('rust/target/async-continuation-reference-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/helper.ts');writeFileSync(path,code);const session=projectReadSession(root),state=session.get(path,code);assert.deepEqual(state.errors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')})),[]);
  const transformed=transformAsyncContinuations(code,path,session),runtime=new URL('./native-read-runtime-v5.mjs',import.meta.url);
  const file=join(root,'observed.mjs');writeFileSync(file,transformed.code.replaceAll('/@fs'+runtime.pathname,runtime.href));
  const plainFile=join(root,'plain.mjs');writeFileSync(plainFile,ts.transpileModule(code,{compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText);
  return {root,path,code,state,transformed,observed:await import(pathToFileURL(file)),plain:await import(pathToFileURL(plainFile)),site:{path:join(root,'src/main.tsx'),sourceSha256:'reference-original-source',start:1,projectRevision:state.revision}};
}
const cases=[
  ['assignment','box.value=2;return box.value;'],
  ['compound assignment','box.value+=2;return box.value;'],
  ['logical assignment','box.value&&=3;return box.value;'],
  ['prefix increment','return ++box.value;'],
  ['postfix increment','return box.value++;'],
  ['array destructuring','[box.value]=[4];return box.value;'],
  ['object destructuring','({n:box.value}={n:5});return box.value;'],
  ['for-of assignment','for(box.value of [2,3]){}return box.value;'],
  ['parenthesized type assertion','(box.value as number)=6;return box.value;'],
  ['non-null update','return box.value!++;'],
];
for(const[name,body]of cases)test('reference semantics survive '+name,async()=>{
  const row=await input('export async function consume(){const trace:string[]=[];let value=1;const box={get value(){trace.push("get");return value;},set value(next:number){trace.push("set:"+next);value=next;}};await Promise.resolve();'+body.replace('return ','const result=')+'return JSON.stringify({result,value,trace});}');
  assert.equal(await nativeReads.candidate(row.site,()=>row.observed.consume()),await row.plain.consume());
  assert(row.transformed.open.some(gap=>/retain its reference/.test(gap.reason)));
});
test('delete retains the original property reference',async()=>{const row=await input('export async function consume(){const box:{value?:number}={value:1};await Promise.resolve();const deleted=delete (box.value);return JSON.stringify({deleted,present:"value" in box});}');assert.equal(await row.observed.consume(),await row.plain.consume());assert(row.transformed.open.some(gap=>/retain its reference/.test(gap.reason)));});
test('tagged templates retain their original method receiver',async()=>{const row=await input('export async function consume(){const box={value:7,tag(parts:TemplateStringsArray){return this.value+parts.length;}};await Promise.resolve();return (box.tag)`x`;}');assert.equal(await row.observed.consume(),8);assert.equal(await row.plain.consume(),8);assert(row.transformed.open.some(gap=>/retain its reference/.test(gap.reason)));});
test('ordinary method calls retain their receiver',async()=>{const row=await input('export async function consume(){const box={value:7,read(){return this.value;}};await Promise.resolve();return box.read();}');assert.equal(await row.observed.consume(),7);assert.equal(await row.plain.consume(),7);});
test('a read on the right of a property write keeps native observation',async()=>{
  const row=await input('export async function consume(read:()=>number){const box={value:0};await Promise.resolve();box.value=read();return box.value;}');const node={},read=()=>nativeReads.finish(4,nativeReads.begin(node,{},()=>null,()=>null));nativeReads.tag(read,node,{});
  assert.equal(await nativeReads.candidate(row.site,()=>row.observed.consume(read)),4);assert.equal(await row.plain.consume(()=>4),4);assert.equal(nativeReads.events.filter(event=>event.site.path===row.site.path).length,1);
});
