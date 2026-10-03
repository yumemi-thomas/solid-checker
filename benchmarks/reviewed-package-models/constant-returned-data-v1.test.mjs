import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {constantReturnedData} from './constant-returned-data-v1.mjs';
const positives=[
  ['discarded read and counter','read();h.tick();return {value:9};'],
  ['delay','await Promise.resolve();read();return {value:9};'],
  ['same branches','if(read())return {value:9};else return {value:9};'],
  ['throwing branch','if(read())throw Error("expected");return {value:9};'],
  ['finally replaces read result','try{return {value:read()};}finally{return {value:9};}'],
  ['finally preserves data','try{read();return {value:9};}finally{h.tick();}'],
  ['catch agrees','try{read();return {value:9};}catch{return {value:9};}'],
  ['nested return stays local','const inner=()=>({value:read()});inner();return {value:9};'],
  ['fields agree despite order','if(read())return {value:9,label:"done"};return {label:"done",value:9};'],
  ['literal values','read();return {value:-0,large:9n,empty:null,ready:false,label:`done`,missing:void 0};'],
  ['unreachable return','read();return {value:9};return {value:read()};'],
];
const negatives=[
  ['read result','return {value:read()};'],
  ['different branch values','if(read())return {value:9};return {value:10};'],
  ['different field sets','if(read())return {value:9};return {value:9,extra:0};'],
  ['fallthrough','if(read())return {value:9};'],
  ['conditional finally','try{return {value:read()};}finally{if(read())return {value:9};}'],
  ['finally overrides data','try{return {value:9};}finally{return {value:read()};}'],
  ['Promise adoption','read();return Promise.resolve({value:9});'],
  ['awaited object','read();return await {value:9};'],
  ['own then','read();return {value:9,then:undefined};'],
  ['prototype','read();return {__proto__:null,value:9};'],
  ['accessor','return {get value(){return read();}};'],
  ['method','return {value:9,method(){return read();}};'],
  ['spread','return {...{value:9}};'],
  ['computed key','return {["value"]:9};'],
  ['named allocation','const result={value:9};read();return result;'],
  ['constant binding','const nine=9;read();return {value:nine};'],
  ['nested data','read();return {value:{nested:9}};'],
  ['unknown loop','while(read())return {value:read()};return {value:9};'],
  ['switch','switch(read()){case 1:return {value:9};default:return {value:9};}'],
  ['signed zero differs','if(read())return {value:-0};return {value:0};'],
  ['direct eval','eval("read()");return {value:9};'],
  ['field budget',`return {${Array.from({length:17},(_,i)=>'p'+i+':9').join(',')}};`],
  ['statement budget',Array.from({length:260},()=> 'read();').join('')+'return {value:9};'],
];
const root=resolve(mkdtempSync('rust/target/constant-returned-data-unit-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
const paths=new Map();
for(const [kind,cases]of [['positive',positives],['negative',negatives]])for(const [index,[name,body]]of cases.entries()){
  const path=join(root,'src',kind+index+'.ts');paths.set(name,path);writeFileSync(path,`import type {Accessor} from 'solid-js';export async function callback(read:Accessor<number>,h:{tick():void}){${body}}`);
}
const options={target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,strict:true,skipLibCheck:true,noEmit:true};
const program=ts.createProgram([...paths.values()],options);const errors=ts.getPreEmitDiagnostics(program);assert.deepEqual(errors.map(row=>({code:row.code,message:ts.flattenDiagnosticMessageText(row.messageText,'\n')})),[]);
function model(name,alter=()=>{}){const source=program.getSourceFile(paths.get(name)),fn=source.statements.find(ts.isFunctionDeclaration),meta={path:source.fileName,start:fn.getStart(source),end:fn.end,sha256:hash(source.text)};alter(meta);return constantReturnedData(program,meta);}
for(const [name]of positives)test(name+' preserves only own-field initializer facts',()=>{const result=model(name);assert(result);assert.equal(result.kind,'source-constant-own-data-body-return');assert.equal(result.scope,'own-field-initializers-on-normal-body-return');assert.equal(result.promiseSettlement,'unobserved');assert.equal(result.objectIdentity,'unproved');assert.equal(result.effects,'open');assert.equal(result.mutationAfterReturn,'open');assert.equal(result.authority,false);assert.equal(result.certification,false);});
for(const [name]of negatives)test(name+' leaves result content open',()=>assert.equal(model(name),null));
test('source hash mismatch stays open',()=>assert.equal(model('delay',meta=>{meta.sha256=hash('different');}),null));
test('span mismatch stays open',()=>assert.equal(model('delay',meta=>{meta.end++;}),null));
