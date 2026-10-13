// Counts are against supplied benchmark assertions, not real-application accuracy.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
import originalCases from './noise-zero-replay-cases-v1.mjs';
import freshCases from './noise-zero-fresh-cases-v2.mjs';

const out=resolve(process.argv[2]);assert(!existsSync(out));
const paths={
  previous:'rust/target/warning-accuracy-combined-summary-v1.json',
  original:'rust/target/noise-zero-replay-reads-v2/results.json',
  originalAudit:'rust/target/noise-zero-replay-audit-v1.json',
  fresh:'rust/target/noise-zero-fresh-reads-v1/results.json',
  freshAudit:'rust/target/noise-zero-fresh-audit-v1.json',
  originalCapture:'rust/target/noise-zero-capture-audit-v1.json',
  freshCapture:'rust/target/noise-zero-fresh-capture-audit-v1.json',
  originalAssertions:'rust/target/noise-zero-assertion-feedback-v1.json',
  originalAssertionAudit:'rust/target/noise-zero-assertion-audit-v1.json',
  freshAssertions:'rust/target/noise-zero-fresh-assertion-feedback-v1.json',
  freshAssertionAudit:'rust/target/noise-zero-fresh-assertion-audit-v1.json',
  intent:'rust/target/noise-zero-intent-boundary-v1.json',
  typing:'rust/target/noise-zero-published-typing-v1/results.json',
  typingBoundary:'rust/target/noise-zero-typing-audit-v1.json',
  rejectedTyping:'rust/target/warning-accuracy-rejected-typing-v1/results.json',
  adversarial:'rust/target/noise-zero-adversarial-audit-v1.json',
  firstFreeze:'rust/target/noise-zero-detector-freeze-v1.json',
  finalFreeze:'rust/target/noise-zero-detector-freeze-v2.json',
  previousHandoff:'rust/target/warning-accuracy-handoff-freeze-v1.json',
};
const reports=Object.fromEntries(Object.entries(paths).map(([key,path])=>[key,read(path)]));
const authenticate=pin=>assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);
function authenticateInputs(value){
  if(!value||typeof value!=='object')return;
  if(typeof value.path==='string'&&typeof value.sha256==='string'){authenticate(value);return;}
  for(const child of Object.values(value))authenticateInputs(child);
}
for(const report of Object.values(reports)){
  assert.equal(report.authority,false);assert.equal(report.certification,false);
  authenticateInputs(report.inputs);authenticateInputs(report.validators);authenticateInputs(report.validator);
}
for(const key of ['firstFreeze','finalFreeze','previousHandoff'])for(const pin of reports[key].files)authenticate(pin);

function rawScore(cases,report,audit){
  const score={consumers:cases.length,targets:0,targetHints:0,controls:0,noisyControls:0,quietControls:0,misses:[],noisy:[]};
  assert.equal(report.results.length,cases.length);
  for(const plan of cases){
    const result=report.results.find(row=>row.id===plan.id);assert(result&&!result.failure);
    assert.equal(result.stages.length,1);const notes=result.stages[0].current.notes;
    assert(notes.every(note=>note.authority===false&&note.certification===false));
    if(plan.stages[0].role==='target'){score.targets++;if(notes.length)score.targetHints++;else score.misses.push(plan.id);}
    else {assert.equal(plan.stages[0].role,'control');score.controls++;if(notes.length){score.noisyControls++;score.noisy.push(plan.id);}else score.quietControls++;}
  }
  for(const key of ['targets','targetHints','controls','quietControls'])assert.equal(score[key],audit.summary[key]);
  assert.equal(score.noisyControls,audit.summary.noisyControls.length);assert.equal(score.misses.length,audit.summary.misses.length);
  return score;
}
function assertionScore(cases,report,audit,capture){
  const ids=new Set(cases.map(row=>row.id));assert.equal(ids.size,cases.length);
  const notes=new Map(report.notes.map(row=>[row.id,row]));assert.equal(notes.size,report.notes.length);
  assert([...notes.keys()].every(id=>ids.has(id)));
  const score={consumers:cases.length,targets:0,targetNotes:0,controls:0,noisyControls:0,quietControls:0};
  for(const plan of cases){
    const note=notes.get(plan.id);
    if(note){
      const replay=capture.rows.find(row=>row.id===plan.id);assert(replay);
      assert.equal(note.expected,String(plan.stages[0].desired));
      assert.notEqual(replay.primary.before,note.expected);assert.equal(replay.primary.after,note.expected);
      assert.equal(note.severity,'info');assert.equal(note.authority,false);assert.equal(note.certification,false);
      assert.equal(note.repairSafety,'unproved');
    }
    if(plan.stages[0].role==='target'){score.targets++;if(note)score.targetNotes++;}
    else {assert.equal(plan.stages[0].role,'control');score.controls++;if(note)score.noisyControls++;else score.quietControls++;}
  }
  for(const key of ['targets','targetNotes','controls','quietControls'])assert.equal(score[key],audit.summary[key]);
  return score;
}
const originalRaw=rawScore(originalCases,reports.original,reports.originalAudit);
const freshRaw=rawScore(freshCases,reports.fresh,reports.freshAudit);
const originalAssertions=assertionScore(originalCases,reports.originalAssertions,reports.originalAssertionAudit,reports.originalCapture);
const freshAssertions=assertionScore(freshCases,reports.freshAssertions,reports.freshAssertionAudit,reports.freshCapture);
const sum=(a,b,keys)=>Object.fromEntries(keys.map(key=>[key,a[key]+b[key]]));
const combinedRaw=sum(originalRaw,freshRaw,['consumers','targets','targetHints','controls','noisyControls','quietControls']);
const combinedAssertions=sum(originalAssertions,freshAssertions,['consumers','targets','targetNotes','controls','noisyControls','quietControls']);
assert.deepEqual(combinedRaw,{consumers:70,targets:32,targetHints:32,controls:38,noisyControls:10,quietControls:28});
assert.deepEqual(combinedAssertions,{consumers:70,targets:32,targetNotes:32,controls:38,noisyControls:0,quietControls:38});
const recovered=reports.previous.remaining.misses;
assert.equal(recovered.length,4);for(const id of recovered)assert(reports.original.results.find(row=>row.id===id).stages[0].current.notes.length);
assert.equal(reports.previous.combined.targetHints.after,20);assert.equal(reports.previous.combined.noisyControls.after,10);
assert.equal(reports.fresh.detectorFrozenBeforeChallenge,true);
assert.equal(reports.freshAudit.populationStatus,'authored-after-current-detector-freeze');
assert.equal(reports.typing.exitCode,0);assert.equal(reports.typing.files,256);
assert.equal(reports.typingBoundary.summary.typingExclusions,2);assert.equal(reports.typingBoundary.summary.observations,0);
assert.equal(reports.rejectedTyping.exitCode,2);assert.deepEqual(reports.rejectedTyping.codes,[2345,2769]);
assert.equal(reports.adversarial.refusals,3);assert(reports.adversarial.rows.every(row=>row.refused&&row.exitCode!==0));
for(const row of reports.adversarial.rows)authenticate(row);
assert.equal(reports.intent.minimumIndistinguishableControlHintsIfBothTargetsDetected,2);
assert(reports.intent.rows.every(row=>row.sourceEqual&&row.behaviorEqual&&row.targetExpected!==row.controlExpected));

const unitPaths=['body-unit-v1','lexical-unit-v2','data-unit-v1','data-audit-unit-v1','entry-map-unit-v1','retention-unit-v2','assertion-unit-v1'].map(name=>'rust/target/noise-zero-'+name+'.log');
const unitChecks=unitPaths.map(path=>{
  const text=readFileSync(path,'utf8'),number=key=>Number(text.match(new RegExp('ℹ '+key+' (\\d+)'))?.[1]);
  const tests=number('tests');assert.equal(number('pass'),tests);assert.equal(number('fail'),0);assert.equal(number('skipped'),0);
  return {path:resolve(path),tests};
});assert.equal(unitChecks.reduce((sum,row)=>sum+row.tests,0),118);
const captures=[reports.originalCapture,reports.freshCapture];
const captureSummary=Object.fromEntries(['proposals','typingClean','taskCounterChanges','thenGetterCounterChanges','changedPrimaryControlResults','changedVisibleControlText'].map(key=>[key,captures.reduce((sum,report)=>sum+report.summary[key],0)]));
assert.deepEqual(captureSummary,{proposals:58,typingClean:58,taskCounterChanges:56,thenGetterCounterChanges:2,changedPrimaryControlResults:2,changedVisibleControlText:4});
const inputs=[...Object.values(paths),...unitPaths,new URL(import.meta.url).pathname,'benchmarks/reviewed-package-models/noise-zero-replay-cases-v1.mjs','benchmarks/reviewed-package-models/noise-zero-fresh-cases-v2.mjs'].map(path=>({path:resolve(path),sha256:hash(readFileSync(path))}));
const plainComparisons=reports.originalAudit.summary.plainComparisons+reports.freshAudit.summary.plainComparisons+captures.reduce((sum,report)=>sum+report.summary.visibleComparisons,0)+reports.typingBoundary.summary.plainComparisons;
assert.equal(plainComparisons,130);
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),
  scope:'research informational feedback against explicit authored primary assertions on correlated queue cases; zero noise applies to the test-assisted channel only',
  original:{automaticBefore:{targetHints:20,targets:24,noisyControls:10,controls:34},automaticAfter:originalRaw,testAssisted:originalAssertions},
  fresh:{automatic:freshRaw,testAssisted:freshAssertions,status:'fresh bindings and optional getter authored after final profile freeze; derived from unexecuted authored forms'},
  combined:{automatic:combinedRaw,testAssisted:combinedAssertions},recoveredTargets:recovered,captureSummary,
  publicTypingFiles:256,typingExclusions:2,plainComparisons,unitChecks,modifiedReportRefusals:3,
  remaining:{automaticNoise:[...originalRaw.noisy,...freshRaw.noisy],automaticZeroNoise:'incompatible with detecting both source-identical target/control pairs without an explicit intent premise',
    testAssistedOpen:reports.originalAssertions.open,repairSafety:'unproved; two captures break previously passing primary assertions',effects:'open; 56/58 task counts change',
    promiseSettlement:'unobserved',mutationAfterReturn:'open',observationCoverage:'partial executed paths only',otherApplicationAssertions:'unmeasured',generalPackageAccuracy:'unmeasured',productionWarnings:'unchanged'},inputs},null,2)+'\n');
console.log(JSON.stringify({combinedRaw,combinedAssertions,recoveredTargets:recovered.length,focusedTests:118,modifiedReportRefusals:3,captureSummary}));
