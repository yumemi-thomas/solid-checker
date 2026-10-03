import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {ts} from './lower.mjs';
import {memoResultFieldUses} from './memo-result-field-uses-v1.mjs';
import {auditMemoResultFieldUses} from './memo-result-field-uses-audit-v1.mjs';
const positives=[
  ['direct field','return result().value;'],
  ['multiple fields','return result().value+result().other;'],
  ['transparent wrappers','return ((result as (()=>{value:number;other:number}))())!.value;'],
  ['local closure','return ()=>result().value;'],
  ['different shadow','function unrelated(result:()=>number){return result();}return result().value;'],
];
const negatives=[
  ['object identity','return identify(result());'],
  ['object escapes','return result();'],
  ['accessor escapes','return result;'],
  ['accessor passed','consume(result);return result().value;'],
  ['shorthand escapes','return {result};'],
  ['accessor alias','const alias=result;return alias().value;'],
  ['object alias','const data=result();return data.value;'],
  ['computed field','return result()["value"];'],
  ['optional field','return result()?.value;'],
  ['optional call','return result?.().value;'],
  ['unknown field','return result().other;'],
  ['write','result().value=10;return result().value;'],
  ['increment','result().value++;return result().value;'],
  ['wrapped write','(result().value)=10;return result().value;'],
  ['destructure','const {value}=result();return value;'],
  ['unused','return 9;'],
];
const root=resolve(mkdtempSync('rust/target/memo-result-field-unit-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
const paths=new Map();
for(const [kind,cases]of [['positive',positives],['negative',negatives]])for(const[index,[name,body]]of cases.entries()){
  const path=join(root,'src',kind+index+'.ts');paths.set(name,path);writeFileSync(path,`import {createMemo} from 'solid-js';declare function identify(object:object):number;declare function consume(read:()=>object):void;export function App(){const result=createMemo(()=>({value:9,other:9}));${body}}`);
}
const top=join(root,'src/top.ts');paths.set('top-level',top);writeFileSync(top,"import {createMemo} from 'solid-js';export const result=createMemo(()=>({value:9}));result().value;");
const options={target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,strict:true,skipLibCheck:true,noEmit:true};
const program=ts.createProgram([...paths.values()],options);assert.deepEqual(ts.getPreEmitDiagnostics(program).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')})),[]);
function get(name,change=()=>{}){const source=program.getSourceFile(paths.get(name));let memo;function visit(node){if(ts.isCallExpression(node)&&node.expression.getText(source)==='createMemo')memo=node;ts.forEachChild(node,visit);}visit(source);const site={memo:{start:memo.getStart(source),end:memo.end}};change(site);return {source,site,result:memoResultFieldUses(program,source,site,[{key:'value'},...(name==='multiple fields'?[{key:'other'}]:[])])};}
for(const [name]of positives)test(name+' keeps exact field consumers',()=>{const {source,site,result}=get(name);assert(result);assert.equal(result.authority,false);assert.equal(result.certification,false);auditMemoResultFieldUses(program,source,site,result,[{key:'value'},...(name==='multiple fields'?[{key:'other'}]:[])]);});
for(const [name]of [...negatives,['top-level']])test(name+' keeps guidance open',()=>assert.equal(get(name).result,null));
test('wrong memo span remains open',()=>assert.equal(get('direct field',site=>site.memo.end++).result,null));
for(const alter of [model=>model.uses=[],model=>model.uses[0].key='other',model=>model.declaration.sha256='bad',model=>model.authority=true])test('independent audit refuses altered field-use evidence '+alter.toString(),()=>{const {source,site,result}=get('direct field');alter(result);assert.throws(()=>auditMemoResultFieldUses(program,source,site,result,[{key:'value'}]));});
