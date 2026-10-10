import assert from 'node:assert/strict';
import { test } from 'node:test';
import { runtimeFeedback } from './extended-runtime-feedback-v2.mjs';
import { runtimeFeedback as frozen } from './extended-runtime-feedback.mjs';
function harness(collector) {
  let listener, stopped=false, path='first', app=true;
  const NativeError=globalThis.Error;
  class RecordedError extends NativeError { constructor() { super(); this.stack = `Error\n    at runtime (file:///runtime/diagnostic.js:1:1)\n${app ? '    at shared (file:///consumer/main.tsx:10:5)\n    at ' + path + ' (file:///consumer/main.tsx:' + (path==='first'?20:30) + ':2)' : ''}`; } }
  const observe={diagnostics:{subscribe(fn){listener=fn;return()=>{stopped=true;};}}};
  const delivered=[];
  const run=collector(observe,{isAppFrame:f=>f.path.startsWith('file:///consumer/'),onFeedback:item=>delivered.push(item)});
  return { run, delivered, setPath(value){path=value;}, noApp(){app=false;}, stopped:()=>stopped,
    emit(code='REACTIVE_WRITE_IN_OWNED_SCOPE') { globalThis.Error=RecordedError;
      try { listener({code,severity:'error',message:'exact runtime diagnostic',kind:'execution'}); } finally {globalThis.Error=NativeError;} } };
}
test('different callers survive a shared nearest callback location',()=>{
  for(const collector of [frozen,runtimeFeedback]){const h=harness(collector);h.emit();h.setPath('second');h.emit();
    assert.equal(h.run.feedback.length,collector===frozen?1:2);
    if(collector===runtimeFeedback){assert.deepEqual(h.run.feedback[0].location,h.run.feedback[1].location);
      assert.notDeepEqual(h.run.feedback[0].consumerFrames,h.run.feedback[1].consumerFrames);assert.equal(h.delivered.length,2);}}
});
test('repeated diagnostics on one path coalesce with a count',()=>{const h=harness(runtimeFeedback);h.emit();h.emit();h.emit();
  assert.equal(h.run.feedback.length,1);assert.equal(h.run.feedback[0].occurrences,3);assert.equal(h.delivered.length,1);
  assert.equal(h.run.feedback[0].certification,false);h.run.clear();h.emit();assert.equal(h.run.feedback[0].occurrences,1);
});
test('unknown consumer path is recorded without inventing attribution',()=>{const h=harness(runtimeFeedback);h.noApp();h.emit();h.emit();
  assert.equal(h.run.feedback.length,1);assert.equal(h.run.feedback[0].location,null);assert.deepEqual(h.run.feedback[0].consumerFrames,[]);
});
test('type and shape diagnostics remain excluded',()=>{const h=harness(runtimeFeedback);
  for(const code of ['MISSING_EFFECT_FN','SYNC_NODE_RECEIVED_ASYNC','INVALID_REFRESH_TARGET'])h.emit(code);
  assert.equal(h.run.feedback.length,0);h.emit('NO_OWNER_CLEANUP');assert.equal(h.run.feedback.length,1);
});
test('execution and advisory claims keep their own category',()=>{const h=harness(runtimeFeedback);h.emit('NO_OWNER_EFFECT');h.emit('EFFECT_WRITES_OWN_SOURCE');
  assert.deepEqual(h.run.feedback.map(f=>f.category),['execution','advisory']);h.run.stop();assert(h.stopped());
});
