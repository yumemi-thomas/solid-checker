import assert from 'node:assert/strict';
import {test} from 'node:test';
import {boundedEvidenceRecords, encodedEvidenceBytes} from './bounded-evidence-records-v1.mjs';
import {createNativeReads} from './native-read-runtime-v11.mjs';
import {createCallbackSlots} from './callback-slot-runtime-v6.mjs';

const revision={sessionId:'bounded-unit',generation:1},site={path:'consumer',start:1,projectRevision:revision};
const helper={function:{path:'helper',start:1},projectRevision:revision},operation={start:2};
function reader(reads,value=3){const node={},read=()=>reads.finish(value,reads.begin(node,{},()=>null,()=>null));reads.tag(read,node,{});return {node,read};}

test('record limits reject invalid capacities',()=>{
  for(const value of [0,-1,NaN,Infinity,1.5,Number.MAX_SAFE_INTEGER+1]){
    assert.throws(()=>boundedEvidenceRecords({maxRecords:value,maxBytes:100}),RangeError);
    assert.throws(()=>createNativeReads({eventBytes:value}),RangeError);
  }
});
test('record history evicts an old entry after the count limit',()=>{
  const removed=[],records=boundedEvidenceRecords({maxRecords:2,maxBytes:100,onRemove:(key,value)=>removed.push([key,value])});
  for(const key of ['a','b','c'])assert(records.set(key,key));
  assert.equal(records.get('a'),undefined);assert.equal(records.get('c'),'c');assert.deepEqual(removed,[['a','a']]);assert.equal(records.stats.evicted,1);
});
test('recent duplicate access keeps its original record alive',()=>{
  const records=boundedEvidenceRecords({maxRecords:2,maxBytes:100});records.set('a',1);records.set('b',2);assert.equal(records.get('a'),1);records.set('c',3);
  assert.equal(records.get('b'),undefined);assert.equal(records.get('a'),1);
});
test('the byte limit can evict records before the count limit',()=>{
  const records=boundedEvidenceRecords({maxRecords:10,maxBytes:10});records.set('a',1,6);records.set('b',2,6);
  assert.equal(records.size,1);assert.equal(records.stats.bytes,6);assert.equal(records.stats.evicted,1);
});
test('an oversized record is refused without deleting a retained entry',()=>{
  const records=boundedEvidenceRecords({maxRecords:1,maxBytes:10});records.set('a',1,3);assert.equal(records.set('a',2,11),false);
  assert.equal(records.get('a'),1);assert.equal(records.stats.refused,1);assert.equal(records.stats.evicted,0);
});
test('UTF-8 accounting includes multibyte text',()=>{
  assert.equal(encodedEvidenceBytes('🙂'),4);const records=boundedEvidenceRecords({maxRecords:2,maxBytes:5});
  assert.equal(records.set('🙂','🙂'),false);assert.equal(records.stats.refused,1);
});
test('recent native evidence survives thousands of completed scopes',()=>{
  const reads=createNativeReads({events:7}),{read}=reader(reads),events=reads.events;
  for(let index=0;index<2000;index++)assert.equal(reads.candidate({...site,start:index},read),3);
  assert.equal(reads.events,events);assert.equal(reads.events.length,7);assert.equal(reads.stats.seenRecords,7);
  assert.deepEqual(reads.events.map(event=>event.site.start),[1993,1994,1995,1996,1997,1998,1999]);
  assert.equal(reads.stats.retention.events.evicted,1993);assert.equal(reads.stats.observationsComplete,false);
});
test('a retained repeated read keeps the exact occurrence count',()=>{
  const reads=createNativeReads({events:2}),{read}=reader(reads);
  for(let index=0;index<2000;index++)reads.candidate(site,read);
  assert.equal(reads.events.length,1);assert.equal(reads.events[0].occurrences,2000);assert.equal(reads.stats.retention.events.evicted,0);
});
test('a reobserved evicted read retains its original weak native identity',()=>{
  const reads=createNativeReads({events:1}),{read}=reader(reads);reads.candidate(site,read);const id=reads.events[0].identity.id;
  reads.candidate({...site,start:2},read);reads.candidate(site,read);
  assert.equal(reads.events[0].identity.id,id);assert.equal(reads.events[0].occurrences,1);assert.equal(reads.stats.retention.events.evicted,2);
});
test('event byte refusal preserves the original result and exposes coverage loss',()=>{
  const reads=createNativeReads({eventBytes:64}),{read}=reader(reads);assert.equal(reads.candidate(site,read),3);
  assert.deepEqual(reads.events,[]);assert.equal(reads.stats.retention.events.refused,1);assert.match(reads.continuationGaps[0].reason,/retention byte/);
});
test('metadata eviction does not revoke an already captured site or alter values',()=>{
  const reads=createNativeReads({metadata:2});let token;
  reads.candidate(JSON.stringify(site),()=>{token=reads.captureContinuation(JSON.stringify(helper));return 1;});
  for(let index=0;index<100;index++)reads.candidate(JSON.stringify({...site,start:index+2}),()=>index);
  const {read}=reader(reads);reads.continuation(token,JSON.stringify(operation),read);reads.continuationReturn(token,3);reads.continuationFinish(token);
  assert.deepEqual(reads.events[0].site,site);assert.equal(reads.events[0].asyncContinuation.completion,'explicit-primitive-normal-return');
  assert(reads.stats.metadataRecords<=2);assert(reads.stats.retention.metadata.evicted>0);
});
test('uncached oversized metadata remains usable as a premise',()=>{
  const reads=createNativeReads({metadataBytes:64}),large={...site,path:'x'.repeat(2000)};
  assert.equal(reads.candidate(JSON.stringify(large),()=>13),13);assert.equal(reads.stats.metadataRecords,0);assert.equal(reads.stats.retention.metadata.refused,1);
});
test('refusal history is bounded and retains its recent reasons',()=>{
  const reads=createNativeReads({gaps:3});
  for(let index=0;index<100;index++)reads.candidate({...site,start:index},()=>reads.captureContinuation({...helper,projectRevision:{...revision,generation:2}}));
  assert.equal(reads.continuationGaps.length,3);assert.deepEqual(reads.continuationGaps.map(gap=>gap.site.start),[97,98,99]);assert.equal(reads.stats.retention.gaps.evicted,97);
});
test('an oversized refusal remains visible in cumulative counters',()=>{
  const reads=createNativeReads({gapBytes:16});reads.candidate(site,()=>reads.captureContinuation({...helper,projectRevision:{...revision,generation:2}}));
  assert.deepEqual(reads.continuationGaps,[]);assert.equal(reads.stats.continuationRefused,1);assert.equal(reads.stats.retention.gaps.refused,1);
});
test('guard histories are bounded without inventing native-node identity',()=>{
  const reads=createNativeReads({guards:2,events:2});
  for(let index=0;index<50;index++)reads.candidate(site,()=>reads.finish(7,reads.beginGuard({index},()=>null)));
  assert.equal(reads.stats.guardRecords,2);assert.equal(reads.stats.retention.guards.evicted,48);
  assert(reads.events.every(event=>event.identity.kind==='package-observer-guard'));assert.equal(new Set(reads.events.map(event=>event.identity.id)).size,2);
});
test('guard byte refusal leaves the original value intact',()=>{
  const reads=createNativeReads({guardBytes:8});assert.equal(reads.candidate(site,()=>reads.finish(7,reads.beginGuard({source:'original'},()=>null))),7);
  assert.deepEqual(reads.events,[]);assert.equal(reads.stats.retention.guards.refused,1);
});
test('eviction preserves explicit intent and owned-read exclusions',()=>{
  const reads=createNativeReads({events:1}),node={},owned=()=>reads.finish(4,reads.begin(node,{},()=>null,()=>({})));reads.tag(owned,node,{});
  reads.candidate(site,owned);const {read}=reader(reads);const previous=reads.enterIntent();try{reads.candidate(site,read);}finally{reads.leaveIntent(previous);}
  assert.deepEqual(reads.events,[]);reads.candidate(site,read);assert.equal(reads.events.length,1);
});
test('throwing scopes do not retain evidence or replace the original exception',()=>{
  const reads=createNativeReads({events:1}),{read}=reader(reads),error=new Error('original');
  assert.throws(()=>reads.candidate(site,()=>{read();throw error;}),value=>value===error);assert.deepEqual(reads.events,[]);
});
test('distinct pending budget exhaustion remains fail-closed',()=>{
  const reads=createNativeReads({events:2}),readers=Array.from({length:65},()=>reader(reads).read);
  assert.equal(reads.candidate(site,()=>{for(const read of readers)read();return 9;}),9);assert.deepEqual(reads.events,[]);assert.match(reads.continuationGaps[0].reason,/distinct/);
});
test('callback metadata eviction retains exact live callback matching',()=>{
  const reads=createNativeReads(),slots=createCallbackSlots(reads,{metadata:2});let value;
  const allocation={fields:[{key:'task'}],projectRevision:revision},invocation={key:'task',projectRevision:revision},definition={function:{path:'callback',start:1},projectRevision:revision};
  const {read}=reader(reads),fn=()=>{const token=slots.enter(fn,JSON.stringify(definition));try{return slots.returned(token,read());}finally{slots.finish(token);}};
  reads.candidate(site,()=>{value=slots.register({task:fn},JSON.stringify(allocation));return value;});
  for(let index=0;index<100;index++)slots.match(()=>0,JSON.stringify({...definition,function:{path:'unused',start:index}}));
  assert.equal(slots.invoke(value,'task',JSON.stringify(invocation),()=>value.task()),3);assert.equal(reads.events.length,1);
  assert.equal(reads.events[0].callbackRegistration.identityMatched,true);assert(slots.stats.metadataRecords<=2);assert(slots.stats.retention.metadata.evicted>0);
});
test('a replaced callback still fails exact matching after metadata eviction',()=>{
  const reads=createNativeReads(),slots=createCallbackSlots(reads,{metadata:1});let value;const allocation={fields:[{key:'task'}],projectRevision:revision};
  const original=()=>1;reads.candidate(site,()=>{value=slots.register({task:original},JSON.stringify(allocation));return value;});value.task=()=>2;
  assert.equal(slots.invoke(value,'task',JSON.stringify({projectRevision:revision}),()=>slots.match(original,{projectRevision:revision})),null);assert.deepEqual(reads.events,[]);
});
test('stats snapshots cannot expand evidence capacity',()=>{
  const reads=createNativeReads({events:1}),{read}=reader(reads),stats=reads.stats;stats.retention.events.maxRecords=1000;
  for(let start=0;start<3;start++)reads.candidate({...site,start},read);assert.equal(reads.events.length,1);assert.equal(reads.stats.retention.events.maxRecords,1);
});
