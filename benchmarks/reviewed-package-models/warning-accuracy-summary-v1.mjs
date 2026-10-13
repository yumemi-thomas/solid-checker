import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
import adaptedCases from './warning-accuracy-replay-cases-v1.mjs';
import freshCases from './warning-accuracy-fresh-cases-v1.mjs';

const out=resolve(process.argv[2]);assert(!existsSync(out));
const paths={
  adapted:'rust/target/warning-accuracy-replay-reads-v2/results.json',
  adaptedAudit:'rust/target/warning-accuracy-replay-audit-v2.json',
  fresh:'rust/target/warning-accuracy-fresh-reads-v2/results.json',
  freshAudit:'rust/target/warning-accuracy-fresh-audit-v2.json',
  originalFrozenFresh:'rust/target/warning-accuracy-fresh-reads-v1/results.json',
  originalFreshAudit:'rust/target/warning-accuracy-fresh-audit-v1.json',
  baseline:'rust/target/warning-accuracy-fresh-baseline-v1/results.json',
  baselineAudit:'rust/target/warning-accuracy-fresh-baseline-audit-v1.json',
  typing:'rust/target/warning-accuracy-published-typing-v1/results.json',
  rejectedTyping:'rust/target/warning-accuracy-rejected-typing-v1/results.json',
  typingBoundary:'rust/target/warning-accuracy-typing-audit-v1.json',
  selectionFreeze:'rust/target/warning-accuracy-detector-freeze-v2.json',
  finalProfileFreeze:'rust/target/warning-accuracy-detector-freeze-v3.json',
};
const reports=Object.fromEntries(Object.entries(paths).map(([key,path])=>[key,read(path)]));
for(const key of ['adapted','fresh','baseline','originalFrozenFresh'])assert(reports[key].finishedAt);
assert.equal(reports.typing.exitCode,0);assert.equal(reports.typing.files,116);assert.equal(reports.typingBoundary.summary.typingExclusions,2);
assert.equal(reports.rejectedTyping.exitCode,2);assert.deepEqual(reports.rejectedTyping.codes,[2345,2769]);assert.equal(reports.rejectedTyping.checkerNotes,0);
assert.equal(reports.originalFrozenFresh.detectorFrozenBeforeChallenge,true);
assert.equal(reports.fresh.detectorFrozenBeforeChallenge,false); // Final refusal guard followed the challenge.
function score(cases,report,notes){
  const result={consumers:cases.length,targets:0,targetHints:0,controls:0,noisyControls:0,quietControls:0,hints:0,misses:[],noisy:[]};
  for(const plan of cases){const row=report.results.find(item=>item.id===plan.id);assert(row&&!row.failure);const stage=row.stages[0],count=notes(stage).length,role=plan.stages[0].role;result.hints+=count;
    if(role==='target'){result.targets++;if(count)result.targetHints++;else result.misses.push(plan.id);}
    else {assert.equal(role,'control');result.controls++;if(count){result.noisyControls++;result.noisy.push(plan.id);}else result.quietControls++;}
  }
  result.hintPrecisionAgainstAuthoredAssertions=result.hints?result.targetHints/result.hints:null;
  result.targetCoverageAgainstAuthoredAssertions=result.targetHints/result.targets;return result;
}
const adapted={before:score(adaptedCases,reports.adapted,stage=>stage.current.readObservations),after:score(adaptedCases,reports.adapted,stage=>stage.current.notes),baselineStatus:'the frozen preceding projector output retained verbatim as readObservations; adapted population'};
const fresh={before:score(freshCases,reports.baseline,stage=>stage.current.notes),after:score(freshCases,reports.fresh,stage=>stage.current.notes),baselineStatus:'paired actual browser run of preceding V21 policy; source selection frozen before challenge; final missing-evidence guard verified afterward'};
for(const item of [adapted,fresh]){assert.equal(item.before.targetHints,item.after.targetHints);assert.deepEqual(item.before.misses,item.after.misses);}
for(const plan of freshCases){
  const old=reports.originalFrozenFresh.results.find(row=>row.id===plan.id).stages[0],current=reports.fresh.results.find(row=>row.id===plan.id).stages[0];
  for(const key of ['sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','feedback','errors','values','visibleText'])assert.deepEqual(current[key],old[key],plan.id+':'+key);
  for(const key of ['notes','suppressed','readObservations'])assert.equal(current.current[key].length,old.current[key].length,plan.id+':'+key);
}
const combined={};for(const key of ['consumers','targets','targetHints','controls','noisyControls','quietControls','hints'])combined[key]={before:adapted.before[key]+fresh.before[key],after:adapted.after[key]+fresh.after[key]};
combined.hintPrecisionAgainstAuthoredAssertions={before:combined.targetHints.before/combined.hints.before,after:combined.targetHints.after/combined.hints.after};
const unitPaths=['data-unit-v1','consumption-unit-v2','data-audit-unit-v2','retention-unit-v1'].map(name=>'rust/target/warning-accuracy-'+name+'.log');
const unitChecks=unitPaths.map(path=>{const text=readFileSync(path,'utf8'),number=key=>Number(text.match(new RegExp('ℹ '+key+' (\\d+)'))?.[1]);const total=number('tests');assert.equal(number('pass'),total);assert.equal(number('fail'),0);assert.equal(number('skipped'),0);return {path:resolve(path),tests:total};});assert.equal(unitChecks.reduce((sum,row)=>sum+row.tests,0),92);
const inputs=[...Object.values(paths),...unitPaths,'benchmarks/reviewed-package-models/warning-accuracy-summary-v1.mjs','benchmarks/reviewed-package-models/warning-accuracy-replay-cases-v1.mjs','benchmarks/reviewed-package-models/warning-accuracy-fresh-cases-v1.mjs'].map(path=>({path:resolve(path),sha256:hash(readFileSync(path))}));
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'research informational-hint filtering on authored serial/concurrent queue consumers; raw observed reads remain available; no proven violation or safe repair',adapted,fresh,combined,unitChecks,typingExclusions:2,publicTypingFiles:116,plainComparisons:reports.adaptedAudit.summary.plainComparisons+reports.freshAudit.summary.plainComparisons+reports.typingBoundary.summary.plainComparisons,
  remaining:{misses:adapted.after.misses,noisyControls:[...adapted.after.noisy,...fresh.after.noisy],intent:'open',effects:'open',promiseSettlement:'unobserved',objectIdentity:'open',mutationAfterReturn:'open',observationCoverage:'partial executed paths only',generalPackageAccuracy:'unmeasured',productionWarnings:'unchanged'},inputs},null,2)+'\n');
console.log(JSON.stringify({combined,tests:92,typingExclusions:2},null,2));
