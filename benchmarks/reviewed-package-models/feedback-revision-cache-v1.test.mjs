import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdtempSync,mkdirSync,symlinkSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import createPlugin from './async-read-transform-v14.mjs';
function fixture(){
  const root=resolve(mkdtempSync('rust/target/feedback-revision-cache-'));mkdirSync(join(root,'src'));symlinkSync(resolve('rust/target/app-import-metric/apps/helge-dev/node_modules'),join(root,'node_modules'),'dir');
  const path=join(root,'src/main.tsx'),helper=join(root,'src/consumer.ts'),code=`import {createMemo} from 'solid-js';import {read} from './consumer';export const result=createMemo(()=>Promise.resolve().then(()=>read()));`;
  writeFileSync(path,code);writeFileSync(helper,'export function read(){return 1;}');writeFileSync(join(root,'tsconfig.json'),JSON.stringify({compilerOptions:{module:'ESNext',moduleResolution:'bundler',skipLibCheck:true},include:['src']}));
  const plugin=createPlugin();plugin.configResolved({root});plugin.transform(code,path);const state=plugin.session.get(path,code),invalidated=[];
  const context=file=>({file,timestamp:1,server:{moduleGraph:{getModulesByFile:path=>new Set([{path}]),invalidateModule:module=>invalidated.push(module.path)}}});
  return {root,path,helper,plugin,state,invalidated,context};
}
test('late unrelated startup events keep a matching revision active',()=>{const row=fixture();row.plugin.handleHotUpdate(row.context(join(row.root,'index.html')));row.plugin.handleHotUpdate(row.context(join(row.root,'feedback-entry.mjs')));assert.equal(row.plugin.session.acceptRevision(row.state.revision).valid,true);assert.deepEqual(row.invalidated,[]);});
test('a helper update retires the cached importer even though importer bytes agree',()=>{const row=fixture();writeFileSync(row.helper,'export function read(){return 9;}');row.plugin.handleHotUpdate(row.context(row.helper));assert.equal(row.plugin.session.acceptRevision(row.state.revision).valid,false);assert(row.invalidated.includes(row.path));});
test('an unrecorded new included file retires a changed listing',()=>{const row=fixture(),path=join(row.root,'src/new.d.ts');writeFileSync(path,'declare const added:number;');assert(!row.plugin.session.inputs().some(input=>input.path===path));row.plugin.handleHotUpdate(row.context(path));assert.equal(row.plugin.session.acceptRevision(row.state.revision).valid,false);assert(row.invalidated.includes(row.path));});
