// Helper/result-flow challenges authored after the async constant proof froze.
// Reuse the real controlled-signal setup; labels are used only for scoring.
import previous from './async-continuation-cases-v1.mjs';
const target=previous.find(row=>row.id==='async-continuation-await-read-target'),control=previous.find(row=>row.id==='async-continuation-await-read-control'),cases=[];
function add(name,{body,role='control',result=role==='control'?'9':'1',desired=role==='control'?result:'2',source=target.source,files={},helper=null}){
  cases.push({...target,id:'async-constant-'+name,source,files,stages:[{id:'initial',helper:helper??'export async function consume(read:()=>number){await Promise.resolve();'+body+'}',role,initial:result,afterUpdate:result,desired}]});
}
for(const[name,body]of[
  ['discarded-read','read();return 9;'],
  ['finally-override','try{return read();}finally{return 9;}'],
  ['nested-finally','try{try{return read();}finally{return 10;}}finally{return 9;}'],
  ['catch-agreement','try{read();return 9;}catch{return 9;}'],
  ['normal-finally','try{read();return 9;}finally{read();}'],
  ['conditional-final-agreement','try{read();return 9;}finally{if(read())return 9;}'],
  ['branch-agreement','if(read())return 9;else return 9;'],
  ['await-literal','read();return await 9;'],
  ['unknown-loop-overridden','try{for(const value of [read()])return value;}finally{return 9;}'],
])add(name+'-control',{body});
for(const[name,body]of[
  ['direct-result','return read();'],
  ['catch-only-constant','try{return read();}catch{return 9;}'],
  ['conditional-final-override','try{return read();}finally{if(read()>100)return 9;}'],
  ['finally-reads-result','try{return 9;}finally{return read();}'],
  ['branch-reads-result','if(read())return read();return 0;'],
  ['unknown-loop-result','for(let n=0;n<1;n++)return read();return 0;'],
])for(const working of[false,true])add(name+'-'+(working?'control':'target'),{body,role:working?'control':'target',source:working?control.source:target.source,desired:'2',...(working?{result:'1'}:{})});
// Working capture controls need the source's dependency to trigger recomputation.
for(const row of cases)if(row.stages[0].role==='control'&&row.source===control.source)row.stages[0].afterUpdate='2';
for(const[name,literal,result]of[['string','"constant"','constant'],['boolean','false','false'],['null','null','null'],['bigint','9n','9'],['undefined','void 0','undefined']])add(name+'-control',{body:'read();return '+literal+';',result});
const constant='export async function consume(read:()=>number){await Promise.resolve();read();return 9;}';
const namespace=target.source.replace("import {consume} from './consumer';","import * as helpers from './consumer';").replace('const invoke=consume;','').replaceAll('invoke(read)','helpers.consume(read)');
add('namespace-control',{source:namespace,helper:constant});
add('reexport-control',{helper:"export {consume} from './inner';",files:{'inner.ts':constant}});
add('arrow-control',{helper:'export const consume=async(read:()=>number)=>{await Promise.resolve();read();return 9;};'});
add('await-whole-control',{helper:constant,source:target.source.replace('()=>invoke(read)','async()=>await invoke(read)')});
for(const working of[false,true]){
  const source=(working?control.source:target.source).replace('function App(){','function App(){let raw=1;').replace('h.update=()=>{','h.update=()=>{raw=2;').replace('()=>invoke(read)','()=>invoke(read).then(()=>raw)');
  add('registration-larger-result-'+(working?'control':'target'),{helper:constant,source,role:working?'control':'target',result:'1',desired:'2'});cases.at(-1).stages[0].afterUpdate=working?'2':'1';
}
// Deliberately supported observations with unresolved source result proofs.
add('constant-variable-control',{body:'const nine=9;read();return nine;'});
add('constant-loop-control',{body:'for(let n=0;n<1;n++)read();return 9;'});
for(const role of['control','target'])add('method-'+role,{role,helper:'const api={async read(value:()=>number){await Promise.resolve();'+(role==='control'?'value();return 9;':'return value();')+'}};export const consume=(read:()=>number)=>api.read(read);'});
add('constant-array-control',{helper:constant,source:target.source.replace('()=>invoke(read)','()=>Promise.all([invoke(read)])')});
add('written-javascript-target',{role:'target',helper:"export {consume} from './inner.js';",files:{'inner.js':'export async function consume(read){await Promise.resolve();read();return 9;}consume=async function(read){await Promise.resolve();return read();};'}});
const invalidSolid=previous.find(row=>row.id==='async-continuation-invalid-solid-typing'),invalidMap=previous.find(row=>row.id==='async-continuation-invalid-map-typing');
for(const row of[invalidSolid,invalidMap])cases.push({...row,id:row.id.replace('async-continuation-','async-constant-')});
export default cases;
