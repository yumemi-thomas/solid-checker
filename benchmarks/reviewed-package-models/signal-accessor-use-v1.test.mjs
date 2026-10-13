import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,writeFileSync,symlinkSync,readFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {ts} from './lower.mjs';
import {packageShortcuts,instrumentPackageShortcuts} from './package-shortcut-v2.mjs';
const install=resolve('rust/target/app-import-metric/apps/helge-dev'),head=`import {getObserver,createSignal} from 'solid-js';`;
function input(body){const root=resolve(mkdtempSync('rust/target/accessor-use-unit-'));symlinkSync(join(install,'node_modules'),join(root,'node_modules'),'dir');const path=join(root,'index.js'),text=head+body;writeFileSync(path,text);return {path,text};}
for(const [label,body,kind]of [
  ['direct getter',`const [get]=createSignal(1);return get();`,'accessor-call'],
  ['getter const alias',`const [get]=createSignal(1);const next=get;return next();`,'accessor-call'],
  ['wrapped getter alias',`const [get]=(((createSignal(1))));const next=((get));return ((next))();`,'accessor-call'],
  ['object shorthand escape',`const [get]=createSignal(1);const data={get};return data.get();`,'accessor-object-escape'],
  ['object renamed escape',`const [get]=createSignal(1);const data={read:get};return data.read();`,'accessor-object-escape'],
  ['escaped getter alias',`const [get]=createSignal(1);const next=get;const data={read:next};return data.read();`,'accessor-object-escape'],
  ['tuple index call',`const pair=createSignal(1);return pair[0]();`,'tuple-index-call'],
  ['tuple const alias',`const pair=createSignal(1);const next=pair;return next[0]();`,'tuple-index-call'],
  ['assigned tuple',`let pair;pair=createSignal(1);return pair[0]();`,'tuple-index-call'],
  ['cached tuple assignment',`let pair=cache.n;if(!pair)cache.n=pair=createSignal(1);return pair[0]();`,'tuple-index-call'],
  ['inline getter',`return createSignal(1)[0]();`,'inline-index-call'],
])test('source fact handles '+label,()=>{const source=input(`function read(){if(!getObserver())return 1;${body}}`),models=packageShortcuts(source.text,source.path).models;assert.equal(models.length,1);assert.equal(models[0].accessorUses[0][0].kind,kind);assert.equal(models[0].valueFlow,'open');assert.equal(models[0].accessorUses[0][0].dispatch,'open');});
for(const [label,body]of [
  ['unused getter',`const [unused]=createSignal(1);return 9;`],
  ['setter only',`const [,set]=createSignal(1);void set;return 9;`],
  ['unused tuple',`const pair=createSignal(1);return 9;`],
  ['only setter called',`const pair=createSignal(1);pair[1](2);return 9;`],
  ['mutable getter alias',`const [get]=createSignal(1);let next=get;next=()=>9;return next();`],
  ['rebound tuple',`let pair=createSignal(1);pair=[()=>9,()=>{}];return pair[0]();`],
  ['overwritten tuple getter',`const pair=createSignal(1);pair[0]=()=>9;return pair[0]();`],
  ['overwritten aliased tuple getter',`const pair=createSignal(1);const next=pair;next[0]=()=>9;return next[0]();`],
  ['nested-only getter',`const [get]=createSignal(1);function later(){return get();}return 9;`],
  ['dynamic tuple key',`const pair=createSignal(1);return pair[key]();`],
  ['optional getter call',`const [get]=createSignal(1);return get?.();`],
  ['getter binding default',`const [get=()=>9]=createSignal(1);return get();`],
  ['shadowed getter name',`const [get]=createSignal(1);{const get=()=>9;return get();}`],
])test('source fact refuses '+label,()=>{const source=input(`function read(){if(!getObserver())return 9;${body}}`);assert.equal(packageShortcuts(source.text,source.path).models.length,0);});
test('object escape alone keeps actual read and result flow open',()=>{
  const source=input(`function read(){if(!getObserver())return 9;const [get]=createSignal(1);const metadata={get};return 9;}`),model=packageShortcuts(source.text,source.path).models[0];assert(model);assert.equal(model.accessorUses[0][0].kind,'accessor-object-escape');assert.equal(model.accessorUses[0][0].resultFlow,'open');
});
test('a discarded getter call still does not establish returned-value flow',()=>{
  const source=input(`function read(){if(!getObserver())return 9;const [get]=createSignal(1);get();return 9;}`),model=packageShortcuts(source.text,source.path).models[0];assert(model);assert.equal(model.valueFlow,'open');
});
test('native owner import is added without reordering module dependencies',()=>{
  const source=input(`function read(){if(!getObserver())return 1;const [get]=createSignal(1);return get();}`),text=`import './before.js';${source.text}`,result=instrumentPackageShortcuts(text,source.path),emitted=ts.createSourceFile(source.path,result.code,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),imports=emitted.statements.filter(ts.isImportDeclaration);
  assert.deepEqual(imports.map(node=>node.moduleSpecifier.text),['./before.js','solid-js']);assert(imports[1].importClause.namedBindings.elements.some(node=>node.propertyName?.text==='getOwner'));
});
test('retained trigger-cache and static-store paths survive the stronger gate',()=>{
  const paths=['/private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-e1VMl0/node_modules/@solid-primitives/trigger/dist/index.js','/private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-0SWsjs/node_modules/@solid-primitives/static-store/dist/index.js'];
  assert.deepEqual(paths.map(path=>packageShortcuts(readFileSync(path,'utf8'),path).models[0].accessorUses[0][0].kind),['accessor-object-escape','tuple-index-call']);
});
test('the existing unused-signal counterexample is no longer enrolled',()=>{
  const path=resolve('rust/target/app-import-metric/apps/package-shortcut-local-controls/node_modules/study-shortcut-control/index.js');assert.equal(packageShortcuts(readFileSync(path,'utf8'),path).models.length,0);
});
