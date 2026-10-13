import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,readFileSync,statSync,utimesSync,symlinkSync,mkdirSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {hash} from '../catalog.mjs';
import {projectTransaction} from './transaction.mjs';
import {programPool} from './program-pool.mjs';
import {completionProtocol,waitForApplication} from './readiness.mjs';
import {processTree} from './memory.mjs';
import {authenticateStage} from './evidence.mjs';
import {inPageEpoch,requirePageEpoch} from './page-epoch.mjs';

function fixture(extra=[]){
  const root=mkdtempSync(join(tmpdir(),'solid-development-')),path=join(root,'main.ts'),code='const value=1;';writeFileSync(path,code);
  const inputs=[{kind:'read',path,exists:true,sha256:hash(code)},...extra],revision={sessionId:'issued',generation:1,invalidation:0,inputSha256:hash(JSON.stringify(inputs))};
  const source={text:code},state={source,program:{getSourceFile:name=>name===path?source:null},revision};
  const session={get:()=>state,inputs:()=>structuredClone(inputs),acceptRevision:rev=>({valid:JSON.stringify(rev)===JSON.stringify(revision)})};
  return {root,path,code,inputs,revision,session};
}
test('one synchronous program serves repeated nested queries with bounded validation',()=>{
  const f=fixture();let validations=0,escaped;
  const projected=projectTransaction(f.session,f.path,f.code,session=>{
    escaped=session;for(let n=0;n<30;n++){assert.equal(session.get(f.path,f.code).source.text,f.code);assert(session.acceptRevision(f.revision).valid);}
    assert(!session.acceptRevision({...f.revision,generation:0}).valid);return {notes:[]};
  },()=>{validations++;return {valid:true};});
  assert.equal(validations,2);assert.equal(projected.metrics.lookups,30);assert.equal(projected.metrics.revisionChecks,31);
  assert.throws(()=>escaped.get(f.path,f.code),/closed/);assert.throws(()=>escaped.acceptRevision(f.revision),/closed/);
});
test('same size and same mtime edits during projection refuse the result',()=>{
  const f=fixture(),old=statSync(f.path);
  assert.throws(()=>projectTransaction(f.session,f.path,f.code,()=>{writeFileSync(f.path,'const value=2;');utimesSync(f.path,old.atime,old.mtime);return {notes:['unsafe']};}),/changed during/);
});
test('unknown input kinds fail closed before projection',()=>{
  const f=fixture([{kind:'unknown',path:'/unknown'}]);let ran=false;
  assert.throws(()=>projectTransaction(f.session,f.path,f.code,()=>{ran=true;}),/before projection/);assert(!ran);
});
test('async projections and invalidation are rejected',()=>{
  const f=fixture();assert.throws(()=>projectTransaction(f.session,f.path,f.code,()=>Promise.resolve()),/synchronously/);
  assert.throws(()=>projectTransaction(f.session,f.path,f.code,session=>session.invalidate()),/Cannot invalidate/);
});
test('source outside the exact program is rejected',()=>{
  const f=fixture();assert.throws(()=>projectTransaction(f.session,f.path,f.code,session=>session.get(join(f.root,'other.ts'),'')),/outside/);
});
test('content pool reuses exact identity and changes key for producer/config identity',()=>{
  const f=fixture(),pool=programPool();let builds=0;const build=()=>({id:++builds});
  const first=pool.get({compiler:'one'},f.inputs,build),second=pool.get({compiler:'one'},f.inputs,build);
  assert.equal(first.value,second.value);assert(second.reused);
  assert(!pool.get({compiler:'two'},f.inputs,build).reused);assert.equal(builds,2);
});
test('content pool refuses same-size changes even when timestamp is restored',()=>{
  const f=fixture(),pool=programPool(),old=statSync(f.path);pool.get({compiler:'one'},f.inputs,()=>1);
  writeFileSync(f.path,'const value=2;');utimesSync(f.path,old.atime,old.mtime);
  assert.throws(()=>pool.get({compiler:'one'},f.inputs,()=>2),/inputs changed/);assert.equal(pool.stats.hits,0);
});
test('negative resolution facts invalidate when missing declaration appears',()=>{
  const f=fixture(),missing=join(f.root,'missing.d.ts');f.inputs.push({kind:'file',path:missing,exists:false});const pool=programPool();
  pool.get({compiler:'one'},f.inputs,()=>1);writeFileSync(missing,'export const v: number;');
  assert.throws(()=>pool.get({compiler:'one'},f.inputs,()=>2),/inputs changed/);
});
test('directory membership is part of program reuse',()=>{
  const f=fixture(),dir=join(f.root,'children');mkdirSync(dir);f.inputs.push({kind:'directories',path:f.root,result:JSON.stringify(['children'])});
  const pool=programPool();pool.get({compiler:'one'},f.inputs,()=>1);mkdirSync(join(f.root,'new'));
  assert.throws(()=>pool.get({compiler:'one'},f.inputs,()=>2),/inputs changed/);
});
test('real path changes are rejected',()=>{
  const f=fixture(),link=join(f.root,'link');symlinkSync(f.path,link);f.inputs.push({kind:'realpath',path:link,result:join(f.root,'wrong')});
  assert.throws(()=>programPool().get({compiler:'one'},f.inputs,()=>1),/inputs changed/);
});
test('changed inputs during program construction never enter the pool',()=>{
  const f=fixture(),pool=programPool();assert.throws(()=>pool.get({compiler:'one'},f.inputs,()=>{writeFileSync(f.path,'const value=2;');return 1;}),/during program/);assert.equal(pool.stats.builds,0);
});
test('program pool bounds retained programs',()=>{
  const f=fixture(),pool=programPool({capacity:1});pool.get({compiler:'one'},f.inputs,()=>1);pool.get({compiler:'two'},f.inputs,()=>2);
  assert.equal(pool.stats.evictions,1);assert(!pool.get({compiler:'one'},f.inputs,()=>1).reused);
});
test('pending protocol uses the supplied Solid implementation',()=>{
  const h={},isPending=()=>false;completionProtocol(h,{isPending});assert.equal(h.checkPending,isPending);
});
test('unsupported completion modes fail closed',async()=>{
  await assert.rejects(waitForApplication({},'unknown'),/No admitted/);
});
test('memory sample excludes unrelated browsers and includes owned subprocesses',()=>{
  const result=processTree('10 1 5 node\n11 10 20 Google Chrome\n12 11 30 Google Chrome Helper\n13 1 90 lightpanda',10);
  assert.equal(result.processes,3);assert.equal(result.totalRssBytes,55*1024);assert.equal(result.browserRssBytes,50*1024);assert.equal(result.browserProcesses,2);
});
test('offline input hash and event revisions are authenticated',()=>{
  const f=fixture(),stage={revision:f.revision,inputManifest:f.inputs,events:[{site:{projectRevision:f.revision}}]};
  assert.equal(authenticateStage(stage).length,1);const wrong=structuredClone(stage);wrong.events[0].site.projectRevision={...wrong.revision,generation:2};
  assert.throws(()=>authenticateStage(wrong));assert.throws(()=>authenticateStage({...stage,revision:{...f.revision,inputSha256:'wrong'}}));
});
test('offline stale source observations are refused',()=>{
  const f=fixture(),stage={revision:f.revision,inputManifest:f.inputs,events:[]};writeFileSync(f.path,'const value=2;');
  assert.throws(()=>authenticateStage(stage),/inputs changed/);assert.equal(readFileSync(f.path,'utf8'),'const value=2;');
});
test('a pool clear releases reusable entries',()=>{
  const f=fixture(),pool=programPool();pool.get({compiler:'one'},f.inputs,()=>1);pool.clear();
  assert(!pool.get({compiler:'one'},f.inputs,()=>2).reused);
});
test('interrupted page execution retries from a clean load and records it',async()=>{
  let runs=0,reloads=0;const retries=[];
  const value=await inPageEpoch({reload:async()=>{reloads++;}},async()=>{if(++runs===1)throw Error('Execution context was destroyed');return 2;},{onRetry:value=>retries.push(value)});
  assert.equal(value,2);assert.equal(reloads,1);assert.equal(retries.length,1);
});
test('page retries are bounded and semantic errors never retry',async()=>{
  let reloads=0;const page={reload:async()=>{reloads++;}};
  await assert.rejects(inPageEpoch(page,async()=>{throw Error('semantic mismatch');}),/semantic/);assert.equal(reloads,0);
  await assert.rejects(inPageEpoch(page,async()=>{throw Error('Execution context was destroyed');}),/context/);assert.equal(reloads,2);
});
test('snapshots from a different page load are refused',()=>{
  assert.throws(()=>requirePageEpoch({loadId:'new'},'old'),error=>error.code==='APP_PAGE_EPOCH_CHANGED');
  assert.equal(requirePageEpoch({loadId:'same'},'same').loadId,'same');
});
