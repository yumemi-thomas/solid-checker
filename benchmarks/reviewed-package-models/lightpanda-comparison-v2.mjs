// Compare authenticated observations. Known volatile locations/revisions are
// normalized; source spans, values, counts and mapped frames stay exact.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {hash,read} from './catalog.mjs';

const [benchmarkArg,outArg]=process.argv.slice(2),benchmarkPath=resolve(benchmarkArg),out=resolve(outArg);assert(!existsSync(out));
const benchmark=read(benchmarkPath);assert(benchmark.finishedAt&&!benchmark.failure);
const authenticate=pin=>assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);
for(const pin of benchmark.inputs)authenticate(pin);
const paths=benchmark.runs.map(row=>({engine:row.engine,path:row.result.path,audit:row.audit.path,pilot:false,wallMs:row.wallMs}));
const reports=paths.map(item=>{
  const report=read(item.path),audit=read(item.audit);assert(report.finishedAt&&audit.finishedAt);
  assert.equal(report.engine.kind,item.engine);assert.equal(report.authority,false);assert.equal(report.certification,false);
  assert(audit.inputs.some(pin=>pin.path===item.path&&pin.sha256===hash(readFileSync(item.path))));
  for(const pin of [...audit.inputs,...audit.validators,...report.inputs.files])authenticate(pin);
  assert.equal(audit.summary.stages,8);assert.equal(audit.summary.targetHints,4);assert.equal(audit.summary.quietControls,2);assert.equal(audit.summary.noisyControls.length,2);
  const release=read('rust/target/lightpanda-release-v1.json');
  if(item.engine==='lightpanda'){
    assert.equal(report.engine.version,'1.0.0');assert.equal(report.engine.sha256,release.assets.find(row=>row.name==='lightpanda-aarch64-macos').digest);
  }
  return {...item,report,audit};
});
function normalize(value,root){
  if(typeof value==='string')return value.replaceAll(root,'$CASE').replace(/http:\/\/127\.0\.0\.1:\d+/g,'$ORIGIN');
  if(Array.isArray(value))return value.map(item=>normalize(item,root));
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).filter(([key])=>key!=='projectRevision').map(([key,item])=>[key,normalize(item,root)]));
  return value;
}
const pick=(object,keys)=>Object.fromEntries(keys.map(key=>[key,object?.[key]??null]));
function groups(reportPath,row){
  assert.equal(row.stages.length,1);const stage=row.stages[0],root=join(dirname(reportPath),row.id);
  assert(!row.failure&&row.pageErrors.length===0&&row.blockedRequests.length===0);
  const facts=stage.events.map(event=>({
    site:event.site,context:event.context,occurrences:event.occurrences,identity:pick(event.identity,['kind','id','premise']),reader:event.nativeRead.premise,
    registration:pick(event.callbackRegistration,['allocation','field','definition','invocation','identityMatched','returnedKind','completion','promiseSettlement','resultFlow']),
    continuation:{...pick(event.asyncContinuation,['completion','promiseSettlement','resultFlow']),chain:event.asyncContinuation.chain.map(link=>pick(link,['helper','operation','returnedKind']))},
  }));
  const mapped=stage.events.map(event=>({
    read:event.originalFrames.filter(Boolean),reader:event.nativeRead.originalFrames.filter(Boolean),creation:event.identity.originalCreationFrames.filter(Boolean),
    registration:event.callbackRegistration.originalRegistrationFrames.filter(Boolean),invocation:event.callbackRegistration.originalInvocationFrames.filter(Boolean),
    entry:event.callbackRegistration.originalEntryFrames.filter(Boolean),helpers:event.asyncContinuation.chain.map(link=>link.originalEntryFrames.filter(Boolean)),
  }));
  const hints=stage.current.notes.map(note=>pick(note,['code','severity','category','channel','basis','witness','authority','certification']));
  const suppressed=stage.current.suppressed.map(item=>({model:item.model,consumption:item.consumption,scope:item.scope,effects:item.effects,reactiveIntent:item.reactiveIntent}));
  return normalize({
    behavior:pick(stage,['sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','values','visibleText','feedback','errors','continuationGaps']),
    sourceFacts:facts,mappedFrames:mapped,feedback:{hints,suppressed},
    runtimeCounts:pick(stage.identityStats,['nodes','functions','nativeReadEntries','nativeStoreEntries','continuationCount','continuationCompleted','continuationRefused','candidateCalls','candidateNormalReturns','eventRecords','gapRecords']),
  },root);
}
const reference=reports[0],comparisons=[];
for(const item of reports.slice(1))for(const row of reference.report.results){
  const other=item.report.results.find(candidate=>candidate.id===row.id);assert(other);
  const before=groups(reference.path,row),after=groups(item.path,other);
  const equal=Object.fromEntries(Object.keys(before).map(key=>[key,JSON.stringify(before[key])===JSON.stringify(after[key])]));
  comparisons.push({engine:item.engine,report:item.path,case:row.id,equal,compatible:Object.values(equal).every(Boolean)});
}
const numericSummary=values=>({values,minimum:Math.min(...values),maximum:Math.max(...values),mean:values.reduce((sum,n)=>sum+n,0)/values.length});
const timing=Object.fromEntries(['chromium','lightpanda'].map(engine=>{
  const measured=reports.filter(item=>item.engine===engine&&!item.pilot);
  return [engine,{wallMs:numericSummary(measured.map(item=>item.wallMs)),recordedBrowserMs:numericSummary(reports.filter(item=>item.engine===engine).map(item=>item.report.timings.totalMs)),navigationMs:numericSummary(measured.map(item=>item.report.timings.navigationMs))}];
}));
const inputs=[benchmarkPath,...paths.flatMap(row=>[row.path,row.audit]),'rust/target/lightpanda-release-v1.json',new URL(import.meta.url).pathname].map(path=>({path:resolve(path),sha256:hash(readFileSync(path))}));
writeFileSync(out,JSON.stringify({authority:false,certification:false,developmentOnly:true,finishedAt:new Date().toISOString(),scope:'eight authored cases, two executions per engine in reversed orders, total-process timings with corrected shutdown; no general compatibility or speed claim',
  totalExecutions:32,compatibleComparisons:comparisons.filter(row=>row.compatible).length,comparisons,timing,
  meanWallReduction:1-timing.lightpanda.wallMs.mean/timing.chromium.wallMs.mean,
  normalization:['exact generated case-directory prefix','local origin port','issued projectRevision objects; independently audited in each report','unmapped null frame entries omitted from mapped-frame lists'],
  remaining:{generalPackageCompatibility:'unmeasured',layout:'unmeasured',longSessions:'unmeasured',memoryBenefit:'unmeasured',runtimeSemanticsAuthority:'research premises',productionBrowser:'unchanged'},inputs},null,2)+'\n');
console.log(JSON.stringify({compatibleComparisons:comparisons.filter(row=>row.compatible).length,totalComparisons:comparisons.length,differences:comparisons.filter(row=>!row.compatible).map(row=>({case:row.case,engine:row.engine,groups:row.equal})),timing,meanWallReduction:1-timing.lightpanda.wallMs.mean/timing.chromium.wallMs.mean},null,2));
