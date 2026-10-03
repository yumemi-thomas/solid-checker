import assert from 'node:assert/strict';
import {test} from 'node:test';
import {createNativeReads} from './native-read-runtime-v9.mjs';
const site={path:'measurement-input',start:1,projectRevision:{sessionId:'unit',generation:1}};
function reader(reads,value=2){const node={},read=()=>reads.finish(value,reads.begin(node,{},()=>null,()=>null));reads.tag(read,node,{});return read;}
test('an entered candidate with no reactive read is distinguished from no entry',()=>{
  const reads=createNativeReads();assert.equal(reads.stats.candidateCalls,0);
  assert.equal(reads.candidate(site,()=>13),13);assert.equal(reads.stats.candidateCalls,1);assert.equal(reads.stats.candidateNormalReturns,1);assert.equal(reads.stats.eventRecords,0);
});
test('a throwing candidate increments entry without claiming normal completion',()=>{
  const reads=createNativeReads(),error=new Error('original');assert.throws(()=>reads.candidate(site,()=>{throw error;}),value=>value===error);
  assert.equal(reads.stats.candidateCalls,1);assert.equal(reads.stats.candidateNormalReturns,0);assert.equal(reads.stats.eventRecords,0);
});
test('nested candidates preserve values and count each scope once',()=>{
  const reads=createNativeReads();assert.equal(reads.candidate(site,()=>reads.candidate({...site,start:2},()=>7)),7);
  assert.equal(reads.stats.candidateCalls,2);assert.equal(reads.stats.candidateNormalReturns,2);assert.equal(reads.stats.eventRecords,0);
});
test('repeated original reads retain their exact count while buffers are measured',()=>{
  const reads=createNativeReads(),read=reader(reads);let calls=0;
  assert.equal(reads.candidate(site,()=>{for(let index=0;index<70;index++){read();calls++;}return calls;}),70);
  assert.equal(reads.events.length,1);assert.equal(reads.events[0].occurrences,70);assert.equal(reads.stats.nativeReadEntries,70);assert.equal(reads.stats.eventRecords,1);assert.equal(reads.stats.seenRecords,1);
});
test('budget refusal remains observable without changing the original return',()=>{
  const reads=createNativeReads(),readers=Array.from({length:65},()=>reader(reads));
  assert.equal(reads.candidate(site,()=>{for(const read of readers)read();return 9;}),9);
  assert.equal(reads.stats.candidateNormalReturns,1);assert.equal(reads.stats.eventRecords,0);assert.equal(reads.stats.gapRecords,1);assert.match(reads.continuationGaps[0].reason,/budget/);
});
test('a returned Promise keeps identity and gains no reaction',async()=>{
  const reads=createNativeReads(),trace=[],promise=Promise.resolve(3);
  const value=reads.candidate(site,()=>promise);assert.equal(value,promise);assert.equal(reads.stats.candidateNormalReturns,1);
  queueMicrotask(()=>trace.push('peer'));promise.then(()=>trace.push('settled'));assert.equal(await value,3);await Promise.resolve();assert.deepEqual(trace,['peer','settled']);
});
test('measurement snapshots do not grant mutation of runtime counters',()=>{
  const reads=createNativeReads(),snapshot=reads.stats;snapshot.candidateCalls=100;snapshot.eventRecords=100;
  assert.equal(reads.stats.candidateCalls,0);assert.equal(reads.stats.eventRecords,0);
});
test('metadata measurement counts retained records rather than parse attempts',()=>{
  const reads=createNativeReads(),encoded=JSON.stringify(site);for(let index=0;index<10;index++)reads.candidate(encoded,()=>1);
  assert.equal(reads.stats.candidateCalls,10);assert.equal(reads.stats.metadataRecords,1);assert.equal(reads.stats.eventRecords,0);
});
