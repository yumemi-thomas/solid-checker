import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { authenticateModel, hash, read } from './catalog.mjs';
import { callbackSites, callbackWriteAt, tokensWithoutWrites } from './callback-sites.mjs';
const [selectionArg, outputArg, ...studyArgs] = process.argv.slice(2), selection = read(resolve(selectionArg)), output = resolve(outputArg), catalog = read(selection.catalogPath);
assert(!existsSync(output)); assert.equal(hash(readFileSync(selection.catalogPath)), selection.catalogSha256);
const results = [], studies = [], authenticated = new Set();
for (const argument of studyArgs) {
  const study = resolve(argument), before = read(join(study,'inputs-before.json')), after = read(join(study,'inputs-after.json')); assert.deepEqual(before.files,after.files);
  for (const input of before.files) { assert.equal(hash(readFileSync(input.path)), input.sha256); assert.equal(hash(readFileSync(join(study,'source-inputs',input.sha256.slice(7)))),input.sha256); }
  const subset = read(before.selectionPath), report = read(join(study,'browser/results.json')); assert(report.finishedAt && report.authority === false);
  assert.equal(hash(readFileSync(subset.parentSelectionPath)),subset.parentSelectionSha256); assert.equal(report.results.length,subset.rows.length*2);
  for (const row of subset.rows) {
    if (!authenticated.has(row.package)) { authenticateModel(catalog.models.find(m=>m.package===row.package),row.project); authenticated.add(row.package); }
    for (const declaration of row.declaration) assert.equal(hash(readFileSync(declaration.path)),declaration.sha256);
    const pair = report.results.filter(r=>r.provenance.export===row.export && r.packagePins[0].package===row.package); assert.equal(pair.length,2);
    const roles = Object.fromEntries(pair.map(r=>[r.provenance.role,r]));
    for (const result of pair) { assert.deepEqual(result.packagePins[0].pins,row.pins); assert.deepEqual(result.provenance.arguments,row.arguments); }
    const admitted = pair.every(r=>!r.publishedTypingErrors.length && !r.excludedBeforeExecution);
    if (!admitted) { results.push({ package:row.package,export:row.export,admitted:false,typingErrors:pair.map(r=>({role:r.provenance.role,errors:r.publishedTypingErrors})) }); continue; }
    for (const result of pair) assert(result.runtime.every(r=>r.version==='2.0.0-rc.9'));
    if (pair.some(r=>!r.attributionEnabled)) { results.push({package:row.package,export:row.export,admitted:true,observationUnavailable:true,
      failures:pair.map(r=>({role:r.provenance.role,harnessFailure:r.harnessFailure??null}))}); continue; }
    const paths = Object.fromEntries(pair.map(r=>[r.provenance.role,join(study,'browser',r.id,'src/main.tsx')]));
    const sites = Object.fromEntries(Object.entries(paths).map(([role,path])=>[role,callbackSites(path)]));
    for (const role of ['read','write']) assert.equal(hash(readFileSync(paths[role])),roles[role].sourceSha256);
    assert.deepEqual(tokensWithoutWrites(sites.read),tokensWithoutWrites(sites.write),'The paired source may differ only by callback writes');
    const samples = roles.read.values?.callbackSamples ?? [], writeSamples = roles.write.values?.callbackSamples ?? [];
    const ownWriteFeedback = roles.write.feedback.filter(f=>f.code==='REACTIVE_WRITE_IN_OWNED_SCOPE').map(f=>({feedback:f,site:callbackWriteAt(f,paths.write,sites.write)}));
    const mapped = ownWriteFeedback.filter(f=>f.site && writeSamples.some(s=>s.id===f.site.id));
    const cleanControl = !roles.read.errors?.length && !roles.read.pageErrors.length && !roles.read.consoleErrors.length && !roles.read.harnessFailure &&
      !roles.read.blockedRequests.length && !roles.read.values?.saturated && !roles.read.feedback.some(f=>f.category==='execution');
    const executed = pair.every(r=>[...(r.observations??[]),...(r.errors??[])].some(o=>o.label==='invoke'));
    const claimedIds = new Set(row.callbackSource.assumptions.filter(a=>a.context==='tracked-compute').map(a=>a.id));
    const sourceComparison = [...claimedIds].map(id=>({id,observed:samples.some(s=>s.id===id),observerPresent:samples.some(s=>s.id===id&&s.tracked)}));
    results.push({package:row.package,export:row.export,admitted:true,executed,controlClean:cleanControl,
      callbackObserved:samples.length>0,observerPresent:samples.some(s=>s.tracked),observedOnlyWithoutObserver:samples.length>0&&!samples.some(s=>s.tracked),
      callbackWriteDiagnosed:mapped.length>0,confirmedWritePair:executed&&cleanControl&&mapped.length>0&&!roles.write.blockedRequests.length,
      mappedCallbackWrites:mapped.map(m=>({id:m.site.id,location:m.feedback.originalLocation})),unattributedWrites:ownWriteFeedback.filter(m=>!m.site).length,
      writeSampleWithoutObserver:writeSamples.some(s=>!s.tracked),sourceComparison,source:row.callbackSource,
      roles:Object.fromEntries(pair.map(r=>[r.provenance.role,{samples:r.values?.callbackSamples??[],saturated:r.values?.saturated??false,
        codes:r.feedback.map(f=>f.code),errors:r.errors??[],pageErrors:r.pageErrors,consoleErrors:r.consoleErrors,blockedRequests:r.blockedRequests,harnessFailure:r.harnessFailure??null}]))});
  }
  studies.push({path:study,frozenFiles:before.files.length,consumers:report.results.length});
}
const history = [...results], byKey = new Map();
for (const row of results) { const key=row.package+'\0'+row.export, prior=byKey.get(key);
  if (prior) assert(prior.observationUnavailable,'A retry must replace a specifically unavailable observation');
  if (!prior || !row.observationUnavailable) byKey.set(key,row);
}
results.splice(0,results.length,...byKey.values()); assert.equal(results.length,selection.rows.length);
const count = predicate=>results.filter(predicate).length, summary={sampledExports:results.length,sampledPackages:new Set(results.map(r=>r.package)).size,
  typedPairs:count(r=>r.admitted),excludedPairs:count(r=>!r.admitted),executedPairs:count(r=>r.executed),callbacksObserved:count(r=>r.callbackObserved),
  observerPresent:count(r=>r.observerPresent),observedOnlyWithoutObserver:count(r=>r.observedOnlyWithoutObserver),cleanReadControls:count(r=>r.controlClean),
  mappedCallbackWriteDiagnostics:count(r=>r.callbackWriteDiagnosed),confirmedWritePairs:count(r=>r.confirmedWritePair),
  confirmedWritePackages:new Set(results.filter(r=>r.confirmedWritePair).map(r=>r.package)).size,
  sourceTrackedCallbacksObserved:results.flatMap(r=>r.sourceComparison??[]).filter(s=>s.observed).length,
  sourceTrackedCallbacksWithoutObservedObserver:results.flatMap(r=>r.sourceComparison??[]).filter(s=>s.observed&&!s.observerPresent).length};
writeFileSync(output,JSON.stringify({authority:false,certification:false,basis:'finite package callback executions with published types and exactly mapped application writes',
  selection:selection.summary,studies,summary,results,...(history.length!==results.length?{observationHistory:history}: {})},null,2)+'\n'); console.log(JSON.stringify(summary,null,2));
