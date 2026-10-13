import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,writeFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {constantReturnedData} from './constant-returned-data-v1.mjs';
import {auditConstantReturnedData} from './constant-returned-data-audit-v2.mjs';
const bodies=['read();return {value:9};','if(read())return {value:9};return {value:9};','try{return {value:read()};}finally{return {value:9};}','try{read();return {value:9};}catch{return {value:9};}'];
const root=resolve(mkdtempSync('rust/target/constant-data-audit-unit-')),paths=bodies.map((body,index)=>{const path=join(root,index+'.ts');writeFileSync(path,`export async function callback(read:()=>number){${body}}`);return path;});
const program=ts.createProgram(paths,{target:ts.ScriptTarget.ESNext,strict:true,noEmit:true});assert.deepEqual(ts.getPreEmitDiagnostics(program),[]);
function evidence(index=0){const source=program.getSourceFile(paths[index]),fn=source.statements[0],model=constantReturnedData(program,{path:source.fileName,start:fn.getStart(source),end:fn.end,sha256:hash(source.text)});return {model,observation:{callbackRegistration:{definition:{kind:'source-async-callback-entry',function:model.function},returnedKind:'object'}}};}
for(const[index]of bodies.entries())test('independent outcomes agree '+index,()=>{const {model,observation}=evidence(index);auditConstantReturnedData(program,model,observation);});
const corruptions=[['wrong fields',(m)=>m.data.fields[0].constant.value='2'],['settlement claim',(m)=>m.promiseSettlement='proved'],['purity claim',(m)=>m.effects='none'],['authority claim',(m)=>m.authority=true],['wrong source',(m)=>m.function.sha256='different'],['wrong syntax',(m)=>m.certificate[0].syntaxKind='ReturnStatement'],['registration mismatch',(_m,o)=>o.callbackRegistration.returnedKind='number']];
for(const[name,change]of corruptions)test('audit refuses '+name,()=>{const {model,observation}=evidence();change(model,observation);assert.throws(()=>auditConstantReturnedData(program,model,observation));});
