// Independently reconstruct retained event accounting. No detector/runtime imports.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash} from './catalog.mjs';
const [readArg,plainArg,browserAuditArg,outArg]=process.argv.slice(2),inputs=[readArg,plainArg,browserAuditArg].map(path=>resolve(path)),out=resolve(outArg);
assert(!existsSync(out));
const [observed,plain,browserAudit]=inputs.map(path=>JSON.parse(readFileSync(path,'utf8')));
assert(observed.finishedAt&&plain.finishedAt&&browserAudit.finishedAt);
assert.equal(observed.detectorFrozenBeforeChallenge,true);
assert(browserAudit.inputs.some(pin=>pin.path===inputs[0]&&pin.sha256===hash(readFileSync(inputs[0]))));
assert(browserAudit.inputs.some(pin=>pin.path===inputs[1]&&pin.sha256===hash(readFileSync(inputs[1]))));
const encoder=new TextEncoder(),bytes=value=>encoder.encode(JSON.stringify(value)).length;
function unmapped(input){
  const event=structuredClone(input);delete event.originalFrames;delete event.nativeRead.originalFrames;delete event.identity.originalCreationFrames;
  if(event.callbackRegistration)for(const name of ['originalRegistrationFrames','originalInvocationFrames','originalEntryFrames'])delete event.callbackRegistration[name];
  for(const link of event.asyncContinuation?.chain??[])delete link.originalEntryFrames;
  return event;
}
function eventKey(event){return [event.site.path,event.site.sourceSha256,event.site.start,event.site.projectRevision??null,event.identity.id,event.storeKey??null,
  event.asyncContinuation?.chain.map(link=>[link.helper.function.path,link.helper.function.sha256,link.helper.function.start,link.operation.start,link.returnedKind])??null,
  event.callbackRegistration?[event.callbackRegistration.registrationId,event.callbackRegistration.definition.function,event.callbackRegistration.invocation.operation]:null];}
const results=[];
for(const row of observed.results){
  const control=plain.results.find(item=>item.id===row.id);assert(control);assert(!row.failure&&!control.failure);
  for(const stage of row.stages){
    const same=control.stages.find(item=>item.id===stage.id);assert(same);
    for(const key of ['initial','afterUpdate','visibleText','values','feedback','errors','publishedTypingErrors'])assert.deepEqual(stage[key],same[key]);
    assert.equal(stage.values.completedCycles,640);assert.equal(stage.values.calls,641);assert.equal(stage.initial,'1');assert.equal(stage.afterUpdate,'1');
    const stats=stage.identityStats,retention=stats.retention;
    for(const name of ['events','metadata','gaps','guards']){
      const entry=retention[name];for(const key of ['records','bytes','maxRecords','maxBytes','evicted','refused'])assert(Number.isSafeInteger(entry[key])&&entry[key]>=0);
      assert(entry.records<=entry.maxRecords);assert(entry.bytes<=entry.maxBytes);
    }
    assert.equal(retention.events.maxRecords,256);assert.equal(retention.events.maxBytes,4*1024*1024);
    assert.equal(stats.eventRecords,stage.events.length);assert.equal(stats.seenRecords,stage.events.length);assert.equal(retention.events.records,stage.events.length);
    assert(retention.events.evicted>0);assert.equal(retention.events.refused,0);assert(stage.events.length>0);
    assert.equal(retention.events.evicted+stage.events.length,641);
    const reconstructedBytes=stage.events.reduce((sum,input)=>{const event=unmapped(input);return sum+encoder.encode(JSON.stringify(eventKey(event))).length+bytes(event);},0);
    assert(reconstructedBytes<=retention.events.bytes);assert(retention.events.bytes<=reconstructedBytes+64*stage.events.length);
    assert.equal(new Set(stage.events.map(event=>JSON.stringify(eventKey(event)))).size,stage.events.length);
    assert(stage.events.every(event=>event.authority===false&&event.certification===false&&event.callbackRegistration?.identityMatched===true&&event.asyncContinuation?.completion==='explicit-normal-async-body-return'));
    assert(stage.events.every(event=>event.occurrences===1));
    const ids=stage.events.map(event=>event.callbackRegistration.registrationId);assert.deepEqual(ids,[...ids].sort((a,b)=>a-b));
    assert(stage.current.notes.length>0);assert.equal(stage.current.observationCoverage.status,'partial');assert.equal(stage.current.observationCoverage.complete,false);
    assert.deepEqual(stage.current.observationCoverage.retention,retention);
    assert(stage.current.open.some(open=>open.losses?.some(loss=>loss.kind==='events'&&loss.evicted===retention.events.evicted)));
    const callback=stage.callbackSlotStats.retention.metadata;assert(callback.records<=callback.maxRecords&&callback.bytes<=callback.maxBytes);
    results.push({id:row.id,cycles:stage.values.completedCycles,taskCalls:stage.values.calls,retainedEvents:stage.events.length,evictedEvents:retention.events.evicted,
      reconstructedBytes,accountedBytes:retention.events.bytes,maxBytes:retention.events.maxBytes,conditionalNotes:stage.current.notes.length,complete:false,
      callbackCacheRecords:callback.records,scope:'unchanged original results with recent conditional notes and explicit discarded history; no application defect or total heap bound claim'});
  }
}
const result={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:inputs.map(path=>({path,sha256:hash(readFileSync(path))})),results};
writeFileSync(out,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(results));
