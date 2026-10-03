import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {authenticate,authenticateObservation} from './evidence.mjs';
import {hash,read} from '../catalog.mjs';
const [benchmarkArg,outArg]=process.argv.slice(2),benchmarkPath=resolve(benchmarkArg),out=resolve(outArg),benchmark=read(benchmarkPath);
assert(!existsSync(out)&&benchmark.finishedAt&&!benchmark.failure);for(const pin of benchmark.inputs)authenticate(pin);
const reports=benchmark.runs.map(row=>{
  authenticate(row.result);authenticate(row.audit);
  const report=authenticateObservation(row.result),audit=read(row.audit.path);assert(audit.finishedAt);
  for(const pin of [...audit.inputs,...audit.validators])authenticate(pin);
  assert.equal(report.engine.kind,row.engine);return {...row,report,auditReport:audit};
});
const pick=(value,keys)=>Object.fromEntries(keys.map(key=>[key,value?.[key]??null]));
function normalize(value,root){
  if(typeof value==='string')return value.replaceAll(root,'$CASE').replace(/http:\/\/127\.0\.0\.1:\d+/g,'$ORIGIN');
  if(Array.isArray(value))return value.map(item=>normalize(item,root));
  if(value&&typeof value==='object')return Object.fromEntries(Object.entries(value).filter(([key])=>key!=='projectRevision').map(([key,item])=>[key,normalize(item,root)]));
  return value;
}
function groups(item,row,stage){
  const frames=event=>({read:event.originalFrames?.filter(Boolean),reader:event.nativeRead?.originalFrames?.filter(Boolean),creation:event.identity.originalCreationFrames?.filter(Boolean),
    registration:event.callbackRegistration?.originalRegistrationFrames?.filter(Boolean),invocation:event.callbackRegistration?.originalInvocationFrames?.filter(Boolean),entry:event.callbackRegistration?.originalEntryFrames?.filter(Boolean),helpers:event.asyncContinuation?.chain.map(link=>link.originalEntryFrames?.filter(Boolean))});
  return normalize({
    behavior:pick(stage,['sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','values','visibleText','feedback','errors','continuationGaps']),
    sourceFacts:stage.events.map(event=>({site:event.site,context:event.context,occurrences:event.occurrences,identity:pick(event.identity,['kind','id','premise']),reader:event.nativeRead?.premise,
      registration:pick(event.callbackRegistration,['allocation','field','definition','invocation','identityMatched','returnedKind','completion','promiseSettlement','resultFlow']),
      continuation:{...pick(event.asyncContinuation,['completion','promiseSettlement','resultFlow']),chain:event.asyncContinuation?.chain.map(link=>pick(link,['helper','operation','returnedKind']))}})),
    mappedFrames:stage.events.map(frames),
    feedback:{notes:stage.current.notes.map(note=>pick(note,['code','severity','category','channel','basis','witness','authority','certification'])),suppressed:stage.current.suppressed.map(note=>pick(note,['reason','model','consumption','scope','effects','reactiveIntent']))},
    runtimeCounts:pick(stage.identityStats,['nodes','functions','nativeReadEntries','nativeStoreEntries','continuationCount','continuationCompleted','continuationRefused','candidateCalls','candidateNormalReturns','eventRecords','gapRecords']),
    retired:stage.retired.map(old=>pick(old,['stage','eventCount','acceptedEvents','notes','suppressed'])),
  },join(dirname(item.result.path),row.id));
}
const reference=reports[0],comparisons=[];
for(const item of reports.slice(1))for(const row of reference.report.results){
  const other=item.report.results.find(candidate=>candidate.id===row.id);assert(other);assert(!row.failure&&!other.failure);assert.equal(row.stages.length,other.stages.length);
  for(const [index,stage]of row.stages.entries()){
    const before=groups(reference,row,stage),after=groups(item,other,other.stages[index]);
    const equal=Object.fromEntries(Object.keys(before).map(key=>[key,JSON.stringify(before[key])===JSON.stringify(after[key])]));
    comparisons.push({engine:item.engine,case:row.id,stage:stage.id,equal,compatible:Object.values(equal).every(Boolean)});
  }
}
const inputs=[benchmarkPath,...reports.flatMap(row=>[row.result.path,row.audit.path]),new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))}));
const result={authority:false,certification:false,developmentOnly:true,scope:'Paired authored-case replay, not all-package compatibility; one timed execution per engine',finishedAt:new Date().toISOString(),
  normalization:['generated case-root prefix','local origin port','independently audited issued projectRevision objects','unmapped null frame entries omitted'],
  totalCaseExecutions:reports.reduce((n,row)=>n+row.report.results.length,0),totalStageExecutions:reports.reduce((n,row)=>n+row.auditReport.summary.stages,0),
  compatibleComparisons:comparisons.filter(row=>row.compatible).length,comparisons,
  engines:reports.map(row=>({engine:row.engine,wallMs:row.wallMs,timings:row.timings,memory:pick(row.memory,['scope','maxTotalRssBytes','maxBrowserRssBytes','errors']),summary:row.auditReport.summary,
    automaticReloads:row.report.results.flatMap(item=>item.stages).filter(stage=>stage.automaticReload).length,explicitReloads:row.report.results.flatMap(item=>item.stages).filter(stage=>stage.explicitReload).length})),inputs};
writeFileSync(out,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({compatibleComparisons:result.compatibleComparisons,totalComparisons:comparisons.length,differences:comparisons.filter(row=>!row.compatible),engines:result.engines},null,2));
