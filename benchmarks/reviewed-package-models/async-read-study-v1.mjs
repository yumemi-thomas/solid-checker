// Authenticate original consumers and project intent hints from read contexts.
// Authored expectations and behavior never enter the hint projector.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash,read,closurePins} from './catalog.mjs';
import {readProgram,transformAsyncReads} from './async-read-transform-v1.mjs';
import {asyncReadFeedback} from './async-read-feedback-v1.mjs';
import {familyFeedback} from './family-feedback-system.mjs';
import {scoreHoldout} from './family-holdout-score.mjs';
import {ts} from './lower.mjs';
const [populationArg,browserArg,outputArg]=process.argv.slice(2),populationPath=resolve(populationArg),browserPath=resolve(browserArg),output=resolve(outputArg);
const population=read(populationPath),browser=read(browserPath),cases=(await import(pathToFileURL(population.caseModule))).default;
assert(browser.finishedAt);assert(!existsSync(output));const detector=read(population.detector.path),checked=new Map();
function pins(items){for(const item of items){if(!checked.has(item.path))checked.set(item.path,hash(readFileSync(item.path)));assert.equal(checked.get(item.path),item.sha256,item.path);}}
pins([population.detector,...detector.files,detector.baseline,...population.files,...population.declarations]);
assert(new Date(detector.frozenAt)<new Date(population.frozenAt));assert(new Date(population.frozenAt)<new Date(browser.startedAt));
const profile=dirname(dirname(browserPath)),before=read(join(profile,'inputs-before.json')),after=read(join(profile,'inputs-after.json'));
assert.deepEqual(before.files,after.files);pins(before.files);for(const pkg of population.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
assert.deepEqual(browser.results.map(row=>row.id).sort(),population.rows.map(row=>row.id).sort());const results=[];
for(const row of browser.results){
  const selected=population.rows.find(item=>item.id===row.id),authored=cases.find(item=>item.id===row.id),path=join(dirname(browserPath),row.id,'src/main.tsx'),text=readFileSync(path,'utf8');
  assert.equal(hash(text),selected.sourceSha256);assert.equal(row.sourceSha256,selected.sourceSha256);assert.equal(row.originalSourceSha256,row.sourceSha256);
  assert.equal(hash(authored.source),selected.sourceSha256);assert.equal(hash(authored.flow.toString()),selected.flowSha256);assert.deepEqual(authored.provenance,selected.provenance);assert.deepEqual(row.provenance,selected.provenance);
  const program=readProgram(text,path),source=program.getSourceFile(path),errors=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
  assert.deepEqual(errors,selected.publishedTypingErrors);assert.deepEqual(row.publishedTypingErrors.map(({code,message})=>({code,message})),errors);
  let hints={notes:[],open:[]};
  if(errors.length){assert(row.excludedBeforeExecution);assert.equal(row.feedback.length,0);}
  else{
    const transformed=transformAsyncReads(text,path);assert.equal(row.sourceInstrumentation.refused.length,0);
    assert(row.sourceInstrumentation.transformed.some(item=>item.path===path&&item.sourceSha256===row.sourceSha256&&item.transformedSha256===hash(transformed.code)&&JSON.stringify(item.sites)===JSON.stringify(transformed.sites)));
    for(const event of row.readTrace){assert(event.site.path===path);assert.equal(event.site.sourceSha256,row.sourceSha256);
      assert(transformed.sites.some(site=>JSON.stringify(site)===JSON.stringify(event.site)));
      assert(event.originalLocation?.path===path);assert(event.originalFrames.some(frame=>frame?.path===path&&frame.sourceSha256===row.sourceSha256));}
    hints=asyncReadFeedback(program,source,row.readTrace);
  }
  const combined=familyFeedback({publishedTypingErrors:errors,feedback:row.feedback,errors:row.errors,pageErrors:row.pageErrors,windowErrors:row.windowErrors});
  combined.feedback.push(...hints.notes);combined.gaps.push(...hints.open);
  const behavior=row.values?.behavior;
  results.push({id:row.id,package:authored.package,provenance:selected.provenance,...combined,
    publishedTypingErrors:errors,harnessFailure:row.harnessFailure??null,
    declaredBehavior:behavior?{...behavior,passed:behavior.actual===behavior.desired}:null,
    readContexts:(row.readTrace??[]).map(event=>({start:event.site.start,context:event.context,occurrences:event.occurrences})),
    sourceSha256:row.sourceSha256,reusedNativeAnalysis:false});
}
const evaluationMapping={rule:'reactive-read-after-await',additionalInformationalCodes:['OBSERVED_MEMO_CALLBACK_UNTRACKED_READ']};
const evaluated=results.map(row=>({...row,provenance:{...row.provenance,codes:[...row.provenance.codes??[],...(row.provenance.rules?.includes(evaluationMapping.rule)?evaluationMapping.additionalInformationalCodes:[])]}}));
const summary=scoreHoldout(evaluated),report={authority:false,certification:false,finishedAt:new Date().toISOString(),
  inputs:[populationPath,browserPath,join(profile,'inputs-before.json'),join(profile,'inputs-after.json')].map(path=>({path,sha256:hash(readFileSync(path))})),
  detectorFrozenBeforePopulation:before.files.filter(pin=>!population.files.some(c=>c.path===pin.path)).every(pin=>detector.files.some(d=>d.path===pin.path&&d.sha256===pin.sha256)),
  evaluationMapping,summary,results};
pins(detector.files);pins(population.files);pins(population.declarations);pins(before.files);
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(summary,null,2));
