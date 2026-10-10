// Independent input-revision, frame and behavior audit. No detector imports.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync,statSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash,read,closurePins} from './catalog.mjs';
import {ts} from './lower.mjs';
const [caseArg,readArg,plainArg,outArg]=process.argv.slice(2),casePath=resolve(caseArg),readPath=resolve(readArg),plainPath=resolve(plainArg),out=resolve(outArg);
assert(!existsSync(out));const cases=(await import(pathToFileURL(casePath))).default,observed=read(readPath),plain=read(plainPath);
assert(observed.finishedAt&&plain.finishedAt);assert.equal(observed.variant,'reads');assert.equal(plain.variant,'plain');
for(const report of[observed,plain]){
  for(const pin of report.inputs.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  assert(report.inputs.files.some(pin=>pin.path===casePath));
  const sealPin=report.inputs.files.find(pin=>pin.path!==casePath),seal=read(sealPin.path);
  for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
  assert.equal(report.detectorFrozenBeforeChallenge,new Date(seal.frozenAt).getTime()<statSync(casePath).mtimeMs);
  assert(new Date(report.startedAt).getTime()>statSync(casePath).mtimeMs);
}
assert.deepEqual(observed.results.map(x=>x.id).sort(),cases.map(x=>x.id).sort());assert.deepEqual(plain.results.map(x=>x.id).sort(),cases.map(x=>x.id).sort());
const summary={stages:0,targets:0,targetHints:0,controls:0,quietControls:0,typingExclusions:0,observations:0,suppressions:0,retiredNonemptyBatches:0,automaticReloads:0,plainComparisons:0,misses:[],noisyControls:[],incorrectEarlierSuppressions:[]},audits=[];
const physical=new Map();
function disk(path){if(!physical.has(path))physical.set(path,ts.sys.readFile(path));return physical.get(path);}
function witness(frame,path,text,span){if(frame?.path!==path||frame.sourceSha256!==hash(text))return false;const source=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true),offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=span.start&&offset<span.end;}
for(const challenge of cases){
  const row=observed.results.find(x=>x.id===challenge.id),control=plain.results.find(x=>x.id===challenge.id);assert(!row.failure&&!control.failure);assert.deepEqual(row.pageErrors,[]);assert.deepEqual(control.pageErrors,[]);assert.deepEqual(row.stages.map(x=>x.id),challenge.stages.map(x=>x.id));
  let source=challenge.source,helper=challenge.stages[0].helper,config=JSON.stringify({compilerOptions:{target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:true,skipLibCheck:true,allowJs:true},include:['src']});
  const root=join(dirname(readPath),challenge.id),path=join(root,'src/main.tsx'),helperPath=join(root,'src/consumer.ts'),configPath=join(root,'tsconfig.json');
  for(const[index,stage]of row.stages.entries()){
    const plan=challenge.stages[index],same=control.stages[index];
    if(index){if(plan.mainComment)source=challenge.source+plan.mainComment;else if(plan.config)config=JSON.stringify(plan.config);else helper=plan.helper;}
    assert.equal(stage.sourceSha256,hash(source));assert.equal(stage.helperSha256,hash(helper));assert.equal(stage.afterUpdate,plan.afterUpdate);assert.equal(stage.desired,plan.desired);
    for(const key of['sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','feedback','errors'])assert.deepEqual(stage[key],same[key],challenge.id+':'+stage.id+':'+key);
    assert.deepEqual(stage.errors,[]);assert.deepEqual(stage.current.notes.map(note=>note.severity),stage.current.notes.map(()=> 'info'));
    summary.stages++;summary.plainComparisons++;if(stage.automaticReload)summary.automaticReloads++;
    if(plan.typingCode){assert(stage.publishedTypingErrors.some(d=>d.code===plan.typingCode));assert.deepEqual(stage.current.notes,[]);assert.deepEqual(stage.current.suppressed,[]);summary.typingExclusions++;}
    else{assert.deepEqual(stage.publishedTypingErrors,[]);if(plan.role==='target'){summary.targets++;assert(!stage.behaviorPassed);if(stage.current.notes.length)summary.targetHints++;else summary.misses.push(challenge.id+':'+stage.id);}else{summary.controls++;assert(stage.behaviorPassed);if(!stage.current.notes.length&&!stage.feedback.length)summary.quietControls++;else summary.noisyControls.push(challenge.id+':'+stage.id);}}
    const manifest=stage.inputManifest;assert.equal(hash(JSON.stringify(manifest)),stage.revision.inputSha256);
    const virtual=new Map([[path,source],[helperPath,helper],[configPath,config],...Object.entries(challenge.files??{}).map(([name,text])=>[join(root,'src',name),text])]);
    for(const input of manifest){
      if(input.kind==='read'){const text=virtual.has(input.path)?virtual.get(input.path):disk(input.path);assert.equal(text!==undefined,input.exists);assert.equal(text===undefined?null:hash(text),input.sha256,input.path);}
      else if(input.kind==='file')assert.equal(ts.sys.fileExists(input.path),input.exists);
      else if(input.kind==='directory')assert.equal(ts.sys.directoryExists(input.path),input.exists);
      else if(input.kind==='realpath')assert.equal(ts.sys.realpath?.(input.path)??input.path,input.result);
      else if(input.kind==='listing')assert.equal(JSON.stringify(ts.sys.readDirectory(...input.arguments)),input.result);
      else if(input.kind==='directories')assert.equal(JSON.stringify(ts.sys.getDirectories(input.path)),input.result);
      else assert.fail('Unknown input kind');
    }
    assert(manifest.some(input=>input.kind==='read'&&input.path===path&&input.sha256===hash(source)));assert(manifest.some(input=>input.kind==='read'&&input.path===helperPath&&input.sha256===hash(helper)));
    for(const event of stage.events){
      assert.deepEqual(event.site.projectRevision,stage.revision);assert.equal(event.site.sourceSha256,hash(source));assert.equal(event.site.path,path);assert.deepEqual(event.context,{observer:false,owner:false});
      const transformed=row.instrumentation.transformed.find(item=>item.path===path&&JSON.stringify(item.session.revision)===JSON.stringify(stage.revision));assert(transformed);
      const {projectRevision,...site}=event.site;assert(transformed.sites.some(item=>JSON.stringify(item)===JSON.stringify(site)));
      assert(event.originalFrames.some(frame=>witness(frame,path,source,site)));
      const reader=event.nativeRead.premise,readerText=disk(reader.path);assert.equal(hash(readerText),reader.sourceSha256);
      const span=event.identity.kind==='package-observer-guard'?reader.returned:reader.entry;
      assert(event.nativeRead.originalFrames.some(frame=>witness(frame,reader.path,readerText,span)));
      summary.observations++;
    }
    assert.equal(stage.current.acceptedEvents,stage.events.length);assert.deepEqual(stage.current.revision,stage.revision);
    for(const suppressed of stage.current.suppressed){assert.equal(suppressed.model.scope,'normal-completion-value-only');assert.equal(suppressed.model.authority,false);assert.equal(suppressed.model.certification,false);assert.equal(suppressed.model.function.path,helperPath);assert.equal(suppressed.model.function.sha256,hash(helper));assert.equal(suppressed.model.constant.value,'9');summary.suppressions++;}
    for(const old of stage.retired){const prior=row.stages.find(s=>s.id===old.stage);assert(prior&&row.stages.indexOf(prior)<index);assert.equal(old.acceptedEvents,0);assert.deepEqual(old.notes,[]);assert.deepEqual(old.suppressed,[]);assert.equal(old.eventCount,prior.events.length);if(old.eventCount){assert.notDeepEqual(prior.revision,stage.revision);assert(old.open.every(item=>/retired|issued/.test(item.reason)));summary.retiredNonemptyBatches++;}}
    if(plan.id==='constant-helper'&&stage.earlierProjectionOfInitialEvents?.suppressed.length)summary.incorrectEarlierSuppressions.push(challenge.id);
    audits.push({consumer:challenge.id,stage:stage.id,revision:stage.revision,recordedInputs:manifest.length,events:stage.events.length,oldBatches:stage.retired.filter(x=>x.eventCount).length});
  }
}
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'independent input revisions, issued transform correspondence, mapped read frames, retired batches and plain behavior; upstream semantic models remain research premises',
  populationStatus:observed.populationStatus,inputs:[casePath,readPath,plainPath].map(path=>({path,sha256:hash(readFileSync(path))})),summary,audits},null,2)+'\n');console.log(JSON.stringify(summary,null,2));
