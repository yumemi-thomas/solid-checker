// Authenticate enrollment and native creation; authored labels only score results.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash,read,closurePins} from './catalog.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformNativeReads} from './async-read-transform-v8.mjs';
import {nativeReadFeedback} from './native-read-feedback-v3.mjs';
import {familyFeedback} from './family-feedback-system.mjs';
import {ts} from './lower.mjs';
const [populationArg,browserArg,outputArg]=process.argv.slice(2),populationPath=resolve(populationArg),browserPath=resolve(browserArg),output=resolve(outputArg);
const population=read(populationPath),browser=read(browserPath),detector=read(population.detector.path),cases=(await import(pathToFileURL(population.caseModule))).default;
assert(browser.finishedAt&&!existsSync(output));
function pins(items){for(const item of items)assert.equal(hash(readFileSync(item.path)),item.sha256,item.path);}
pins([population.detector,...detector.files,detector.baseline,...population.files,...population.declarations]);
assert(new Date(detector.frozenAt)<new Date(population.frozenAt));assert(new Date(population.frozenAt)<new Date(browser.startedAt));
const profile=dirname(dirname(browserPath)),before=read(join(profile,'inputs-before.json')),after=read(join(profile,'inputs-after.json'));
const {finishedAt,...stableAfter}=after;assert(finishedAt);assert.equal(before.variant,'reads');assert.deepEqual(before,stableAfter);
pins(before.files);for(const pkg of before.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
for(const pkg of population.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
assert.deepEqual(browser.results.map(row=>row.id).sort(),population.rows.map(row=>row.id).sort());
const results=[];
for(const row of browser.results){
  assert.deepEqual(row.packageSourceProfile,{dependencyDiscovery:false});
  const frozen=population.rows.find(item=>item.id===row.id),authored=cases.find(item=>item.id===row.id),root=join(dirname(browserPath),row.id),path=join(root,'src/main.tsx'),text=readFileSync(path,'utf8');
  assert.equal(hash(text),frozen.sourceSha256);assert.equal(row.sourceSha256,frozen.sourceSha256);assert.equal(hash(authored.source),frozen.sourceSha256);
  assert.equal(hash(authored.flow.toString()),frozen.flowSha256);assert.deepEqual(row.provenance,frozen.provenance);assert.deepEqual(authored.provenance,frozen.provenance);
  assert.deepEqual(row.additionalSourcePins,frozen.additionalSourcePins);
  for(const pin of frozen.additionalSourcePins)assert.equal(hash(readFileSync(join(root,'src',pin.name))),pin.sha256);
  const session=projectReadSession(root),state=session.get(path,text),errors=state.errors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
  assert.deepEqual(errors,frozen.publishedTypingErrors);assert.deepEqual(errors,row.publishedTypingErrors.map(({code,message})=>({code,message})));
  let hints={notes:[],open:[]};
  if(errors.length){assert(row.excludedBeforeExecution);assert.equal(row.feedback.length,0);}
  else{
    const transformed=transformNativeReads(text,path,session);assert.deepEqual(row.sourceInstrumentation.refused,[]);assert.deepEqual(row.sourceInstrumentation.native.refused,[]);
    assert.deepEqual(row.sourceInstrumentation.shortcuts.refused,[]);
    for(const event of row.identityTrace??[])if(event.identity.kind==='package-observer-guard')assert(row.sourceInstrumentation.shortcuts.transformed.some(model=>JSON.stringify(model)===JSON.stringify(event.identity.premise)));
    assert(row.sourceInstrumentation.native.transformed.length>0);
    assert(row.sourceInstrumentation.transformed.some(item=>item.path===path&&item.transformedSha256===hash(transformed.code)&&JSON.stringify(item.sites)===JSON.stringify(transformed.sites)));
    hints=nativeReadFeedback(state.program,state.source,row.identityTrace??[]);
  }
  const native=familyFeedback({publishedTypingErrors:errors,feedback:row.feedback,errors:row.errors,pageErrors:row.pageErrors,windowErrors:row.windowErrors});
  const behavior=row.values?.behavior;results.push({id:row.id,package:row.package??authored.package,provenance:row.provenance,
    typingExcluded:!!errors.length,harnessFailure:row.harnessFailure??null,hints:hints.notes,open:hints.open,
    nativeFeedback:native.feedback,nativeGaps:native.gaps,identityStats:row.identityStats??null,
    behavior:behavior?{...behavior,passed:behavior.actual===behavior.desired}:null,sourceSha256:row.sourceSha256});
}
const targets=results.filter(row=>row.provenance.role==='target'&&!row.typingExcluded),controls=results.filter(row=>row.provenance.role==='control'&&!row.typingExcluded),summary={
  targets:targets.length,targetHints:targets.filter(row=>row.hints.length).length,controls:controls.length,quietControls:controls.filter(row=>!row.hints.length&&!row.nativeFeedback.length).length,
  quietNewHintControls:controls.filter(row=>!row.hints.length).length,newHintNoisyControls:controls.filter(row=>row.hints.length).map(row=>row.id),
  typingExclusions:results.filter(row=>row.typingExcluded).length,misses:targets.filter(row=>!row.hints.length).map(row=>row.id),
  noisyControls:controls.filter(row=>row.hints.length||row.nativeFeedback.length).map(row=>row.id),
  failedBehavior:results.filter(row=>row.behavior&&!row.behavior.passed).map(row=>row.id),harnessFailures:results.filter(row=>row.harnessFailure).map(row=>row.id)};
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:[populationPath,browserPath,...['inputs-before.json','inputs-after.json'].map(name=>join(profile,name))].map(path=>({path,sha256:hash(readFileSync(path))})),
  detectorFrozenBeforePopulation:before.files.filter(pin=>pin.path!==population.caseModule).every(pin=>detector.files.some(old=>old.path===pin.path&&old.sha256===pin.sha256)),summary,results};
pins(detector.files);pins(population.files);pins(before.files);writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(summary,null,2));

