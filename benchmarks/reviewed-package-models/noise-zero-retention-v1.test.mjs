import assert from 'node:assert/strict';
import {test} from 'node:test';
import {nativeReadFeedback} from './native-read-feedback-v28.mjs';
const session={get(){return {revision:{sessionId:'ledger-unit',generation:1}};}},row={records:0,bytes:0,maxRecords:256,maxBytes:4096,evicted:0,refused:0};
function stats(){return {observationsComplete:false,eventRecords:0,seenRecords:0,metadataRecords:0,gapRecords:0,guardRecords:0,retention:Object.fromEntries(['events','metadata','gaps','guards'].map(key=>[key,{...row}]))};}
const malformed=[
  ['missing ledger',()=>undefined],
  ['claims completeness',()=>({...stats(),observationsComplete:true})],
  ['negative loss',()=>{const value=stats();value.retention.events.evicted=-1;return value;}],
  ['excess records',()=>{const value=stats();value.retention.events.records=257;return value;}],
  ['excess bytes',()=>{const value=stats();value.retention.metadata.bytes=4097;return value;}],
  ['fractional count',()=>{const value=stats();value.retention.gaps.refused=.5;return value;}],
  ['missing guard ledger',()=>{const value=stats();delete value.retention.guards;return value;}],
  ['changed event count',()=>({...stats(),eventRecords:1})],
  ['changed deduplication count',()=>({...stats(),seenRecords:1})],
  ['changed metadata count',()=>({...stats(),metadataRecords:1})],
  ['changed gap count',()=>({...stats(),gapRecords:1})],
  ['changed guard count',()=>({...stats(),guardRecords:1})],
];
for(const [name,change]of malformed)test(`retention feedback rejects ${name}`,()=>{
  const result=nativeReadFeedback(session,'unused','',[],change());assert.deepEqual(result.notes,[]);assert.deepEqual(result.suppressed,[]);assert.equal(result.acceptedEvents,0);assert.equal(result.observationCoverage.complete,false);assert.equal(result.observationCoverage.status,'unavailable');assert.equal(result.open.length,1);
});
test('an observation absent from the retained ledger remains closed',()=>{
  const result=nativeReadFeedback(session,'unused','',[{}],stats());assert.equal(result.acceptedEvents,0);assert.match(result.open[0].reason,/counts/);
});

import {ts} from './lower.mjs';
const text='export {};',path='/retention-empty-unit.ts',source=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true),options={target:ts.ScriptTarget.ESNext,types:[]},host=ts.createCompilerHost(options),getSourceFile=host.getSourceFile.bind(host);
host.getSourceFile=(file,...args)=>file===path?source:getSourceFile(file,...args);
const program=ts.createProgram([path],options,host),liveSession={get(){return {program,source,revision:{sessionId:'ledger-unit',generation:1}};},acceptRevision(){return {valid:true};}};
for(const name of ['metadata','guards'])test(`${name} cache eviction leaves issued observations usable`,()=>{
 const ledger=stats();ledger.retention[name].evicted=100;ledger.retention[name].refused=name==='metadata'?2:0;
 const result=nativeReadFeedback(liveSession,path,text,[],ledger);assert.equal(result.observationCoverage.status,'retained');assert.equal(result.observationCoverage.complete,false);assert.deepEqual(result.open,[]);
});
test('discarded refusal history is explicit coverage loss',()=>{
 const ledger=stats();ledger.retention.gaps.evicted=5;const result=nativeReadFeedback(liveSession,path,text,[],ledger);assert.equal(result.observationCoverage.status,'partial');assert.deepEqual(result.notes,[]);assert.equal(result.open.length,1);assert.deepEqual(result.open[0].losses,[{kind:'gaps',evicted:5,refused:0}]);
});

import {createNativeReads} from './native-read-runtime-v11.mjs';
test('an actual refused guard identity is reported as coverage loss',()=>{
 const reads=createNativeReads({guardBytes:8});const original=reads.candidate({path:'guard-unit'},()=>reads.finish(7,reads.beginGuard({source:'large'},()=>null)));assert.equal(original,7);assert.deepEqual(reads.events,[]);
 const result=nativeReadFeedback(liveSession,path,text,reads.events,reads.stats);assert.equal(result.observationCoverage.status,'partial');assert.equal(result.observationCoverage.complete,false);assert.deepEqual(result.notes,[]);assert.deepEqual(result.open.at(-1).losses,[{kind:'guards',evicted:0,refused:1}]);
});

test('changed revision refuses the additional source projection',()=>{const result=nativeReadFeedback({...liveSession,acceptRevision(){return {valid:false};}},path,text,[],stats());assert.equal(result.acceptedEvents,0);assert.deepEqual(result.notes,[]);assert.deepEqual(result.suppressed,[]);assert.match(result.open.at(-1).reason,/revision changed/);});

const closedSession={get(){return {revision:{sessionId:'new-layer-refusal',generation:1}};}};
for(const ledger of [undefined,{observationsComplete:true},{observationsComplete:false,retention:{}},{observationsComplete:false,retention:{events:{records:-1}}}])test('new projector preserves an unavailable ledger refusal '+JSON.stringify(ledger),()=>{const result=nativeReadFeedback(closedSession,'unused','',[],ledger);assert.equal(result.acceptedEvents,0);assert.deepEqual(result.notes,[]);assert.deepEqual(result.suppressed,[]);assert.equal(result.observationCoverage.status,'unavailable');});
