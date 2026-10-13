// Compare authenticated workloads and serialized journal size, not total heap.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash} from './catalog.mjs';
const [boundedAuditArg,unboundedArg,unboundedAuditArg,outArg]=process.argv.slice(2),paths=[boundedAuditArg,unboundedArg,unboundedAuditArg].map(path=>resolve(path)),out=resolve(outArg);
assert(!existsSync(out));const [boundedAudit,unbounded,unboundedAudit]=paths.map(path=>JSON.parse(readFileSync(path)));
for(const pin of boundedAudit.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
assert(unbounded.finishedAt&&unboundedAudit.finishedAt);
assert(unboundedAudit.inputs.some(pin=>pin.path===paths[1]&&pin.sha256===hash(readFileSync(paths[1]))));
const bounded=JSON.parse(readFileSync(boundedAudit.inputs[0].path));
const encoder=new TextEncoder();
const strip=value=>Array.isArray(value)?value.map(strip):value&&typeof value==='object'?Object.fromEntries(Object.entries(value).filter(([key])=>!['originalFrames','originalCreationFrames','originalEntryFrames','originalRegistrationFrames','originalInvocationFrames'].includes(key)).map(([key,value])=>[key,strip(value)])):value;
const bytes=value=>encoder.encode(JSON.stringify(value)).length;
function key(event){return [event.site.path,event.site.sourceSha256,event.site.start,event.site.projectRevision??null,event.identity.id,event.storeKey??null,
  event.asyncContinuation?.chain.map(link=>[link.helper.function.path,link.helper.function.sha256,link.helper.function.start,link.operation.start,link.returnedKind])??null,
  event.callbackRegistration?[event.callbackRegistration.registrationId,event.callbackRegistration.definition.function,event.callbackRegistration.invocation.operation]:null];}
const comparisons=[];
for(const row of bounded.results){
  const old=unbounded.results.find(item=>item.id===row.id);assert(old&&!old.failure);assert.equal(row.stages.length,old.stages.length);
  for(const [index,stage]of row.stages.entries()){
    const previous=old.stages[index],admission=boundedAudit.results.find(item=>item.id===row.id);assert(admission);
    for(const name of ['sourceSha256','helperSha256','initial','afterUpdate','visibleText','values','feedback','errors','publishedTypingErrors'])assert.deepEqual(stage[name],previous[name]);
    assert.equal(stage.current.notes.length,previous.current.notes.length);assert.equal(previous.events.length,641);assert.equal(previous.identityStats.seenRecords,641);
    assert.equal(stage.events.length+stage.identityStats.retention.events.evicted,previous.events.length);
    const oldBytes=previous.events.reduce((sum,input)=>{const event=strip(input);return sum+bytes(key(event))+bytes(event);},0);
    assert(oldBytes>admission.reconstructedBytes);
    comparisons.push({id:row.id,taskCalls:stage.values.calls,previousEvents:previous.events.length,retainedEvents:stage.events.length,previousSerializedBytes:oldBytes,
      retainedSerializedBytes:admission.reconstructedBytes,accountedBytes:admission.accountedBytes,reductionPercent:100*(1-admission.reconstructedBytes/oldBytes),
      currentConditionalNotes:stage.current.notes.length,originalBehaviorMatched:true,complete:false});
  }
}
const result={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:paths.map(path=>({path,sha256:hash(readFileSync(path))})),comparisons,
  limits:['Serialized evidence bytes and fixed record capacities do not bound total JS heap, active scopes or server logs.', 'This is an authored workload, with conditional read hints and no newly demonstrated application defect.', 'Runs overlap other development work and are not isolated performance benchmarks.']};
writeFileSync(out,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(comparisons));
