// Source proposal tests use real installed Solid declarations. Witnesses here
// are supplied unit premises; no native execution or repair safety is claimed.
import assert from 'node:assert/strict';
import {test} from 'node:test';
import {join,resolve} from 'node:path';
import {ts} from './lower.mjs';
import {read,hash} from './catalog.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v2.mjs';
import {captureReplayPlan,applyCaptureReplayPlan} from './capture-replay-plan-v1.mjs';
const root=resolve(read('rust/target/primitives-checkpoint/run-browser.json').results.find(row=>row.package==='@solid-primitives/queue').retainedArtifacts.projectDir),main=join(root,'capture-replay-unit-main.ts'),helper=join(root,'capture-replay-unit-helper.ts');
const revision={sessionId:'source-proposal-unit',generation:1};
function fixture({alias='const read=get;',getter='const get=()=>1;',factory='export function makeTask(read:()=>number){return async()=>{await Promise.resolve();return read();};}',after='',bodyPrefix='',computeSuffix='',nativeRead='read()',factoryCall='makeTask(read)'}={}){
  const source=`import {createMemo} from 'solid-js';import {makeTask} from './capture-replay-unit-helper';function run(task:()=>Promise<number>){return task();}${getter}const value=createMemo(()=>{${bodyPrefix}${alias}return Promise.resolve().then(async()=>{return await run(${factoryCall});});${computeSuffix}});${after}`,virtual=new Map([[main,source],[helper,factory]]),options={target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,strict:true,skipLibCheck:true,noEmit:true},host=ts.createCompilerHost(options),originalRead=host.readFile,originalExists=host.fileExists;
  host.readFile=name=>virtual.has(name)?virtual.get(name):originalRead(name);host.fileExists=name=>virtual.has(name)||originalExists(name);host.getSourceFile=(name,language)=>{const text=host.readFile(name);return text===undefined?undefined:ts.createSourceFile(name,text,language,true);};
  const program=ts.createProgram([main,helper],options,host),mainSource=program.getSourceFile(main),helperSource=program.getSourceFile(helper),sites=nativeReadSites(program,mainSource),site=sites.sites.find(site=>mainSource.text.slice(site.start,site.end)===`run(${factoryCall})`);assert(site);
  const model=asyncContinuationSites(program,helperSource).functions.find(item=>item.operations.some(op=>helperSource.text.slice(op.start,op.end)===nativeRead));assert(model);const operation=model.operations.find(op=>helperSource.text.slice(op.start,op.end)===nativeRead),event={site:{...site,projectRevision:revision},callbackRegistration:{identityMatched:true,definition:{kind:'source-async-callback-entry',function:model.function,projectRevision:revision}},asyncContinuation:{chain:[{helper:{...model,projectRevision:revision},operation}]}};
  return {program,source,event};
}
test('an exact source alias produces only a phase-changing replay proposal',()=>{const f=fixture();assert.deepEqual(ts.getPreEmitDiagnostics(f.program).filter(d=>d.category===ts.DiagnosticCategory.Error),[]);const {plan,open}=captureReplayPlan(f.program,f.event);assert(plan&&!open);assert.equal(plan.repairSafety,'unproved');assert.equal(plan.reactiveIntent,'open');assert.equal(plan.executionPhaseChanged,true);assert.equal(plan.authority,false);assert.equal(plan.certification,false);assert.equal(f.source.slice(plan.edit.start,plan.edit.end),'get');assert.match(applyCaptureReplayPlan(f.source,plan),/const __solidCaptureReplay = \(get\)\(\)/);});
test('transparent getter wrappers survive the proposal',()=>{const f=fixture({alias:'const read=(get satisfies (()=>number));'}),result=captureReplayPlan(f.program,f.event);assert(result.plan);assert.equal(result.plan.edit.from,'(get satisfies (()=>number))');});
test('private capture names do not collide with existing identifiers',()=>{const f=fixture({bodyPrefix:'const __solidCaptureReplay=7;void __solidCaptureReplay;'}),{plan}=captureReplayPlan(f.program,f.event);assert(plan);assert.match(plan.edit.to,/const __solidCaptureReplay_ =/);});
test('same-spelling shadowed helper parameters do not manufacture a source route',()=>{const f=fixture({factory:'export function makeTask(read:()=>number){return async()=>{const read=()=>9;return read();};}'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('a changed helper parameter leaves the proposal open',()=>{const f=fixture({factory:'export function makeTask(read:()=>number){return async()=>{read=()=>9;return read();};}'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('a changed consumer alias leaves the proposal open',()=>{const f=fixture({alias:'let read=get;read=()=>9;'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('getter reassignment leaves the proposal open',()=>{const f=fixture({getter:'let get=()=>1;',after:'get=()=>2;'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('a getter requiring an argument is not invoked by a proposal',()=>{const f=fixture({getter:'const get=(value=1)=>value;'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('generic getters remain open',()=>{const f=fixture({getter:'const get=<T extends number=1>()=>1 as T;'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('object getters remain outside this primitive snapshot profile',()=>{const f=fixture({getter:'const get=()=>({value:1});'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('a nested initializer cannot silently move into the memo compute',()=>{const f=fixture({alias:'{const read=get;',computeSuffix:'}'});assert.deepEqual(ts.getPreEmitDiagnostics(f.program).filter(d=>d.category===ts.DiagnosticCategory.Error),[]);const result=captureReplayPlan(f.program,f.event);assert.equal(result.plan,null);assert.match(result.open.reason,/directly initialized/);});
test('a factory with another statement is not guessed transparent',()=>{const f=fixture({factory:'export function makeTask(read:()=>number){void read;return async()=>{return read();};}'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
test('an exact built-in direct eval leaves bindings open',()=>{const f=fixture({after:'eval("0");'});assert.equal(captureReplayPlan(f.program,f.event).plan,null);});
for(const [name,mutate]of [
  ['source hash',e=>e.site.sourceSha256='changed'],
  ['memo span',e=>e.site.memo.start++],
  ['memo declaration',e=>e.site.memoDeclarations[0].start++],
  ['callback identity',e=>e.callbackRegistration.identityMatched=false],
  ['callback hash',e=>e.callbackRegistration.definition.function.sha256='changed'],
  ['read declaration',e=>e.asyncContinuation.chain[0].operation.declarations[0].start++],
  ['helper revision',e=>e.asyncContinuation.chain[0].helper.projectRevision={...revision,generation:2}],
  ['callback revision',e=>e.callbackRegistration.definition.projectRevision={...revision,generation:2}],
  ['missing chain',e=>e.asyncContinuation.chain=[]],
])test(`inconsistent ${name} refuses the source proposal`,()=>{const f=fixture();mutate(f.event);const result=captureReplayPlan(f.program,f.event);assert.equal(result.plan,null);assert(result.open.reason);});
test('applying a proposal to another source is refused',()=>{const f=fixture(),{plan}=captureReplayPlan(f.program,f.event);assert.throws(()=>applyCaptureReplayPlan(f.source+' ',plan),/source differs/);assert.throws(()=>applyCaptureReplayPlan(f.source,{...plan,edit:{...plan.edit,from:'other'}}),/source differs/);});
