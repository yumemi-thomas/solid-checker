// Supplement independent source/frame audits with exact counts and revision comparisons.
// This imports no detector and grants no certification authority.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
const [mode,currentArg,priorArg,currentAuditArg,priorAuditArg,outArg]=process.argv.slice(2);
assert(['registered','direct','updates'].includes(mode));
const paths=[currentArg,priorArg,currentAuditArg,priorAuditArg].map(value=>resolve(value)),out=resolve(outArg);
assert(!existsSync(out));
const [current,prior,currentAudit,priorAudit]=paths.map(read);
for(const [report,audit,index]of [[current,currentAudit,0],[prior,priorAudit,1]]){
  assert(report.finishedAt&&audit.finishedAt);assert.equal(report.variant,'reads');
  assert.equal(report.authority,false);assert.equal(report.certification,false);
  assert(audit.inputs.some(pin=>pin.path===paths[index]&&pin.sha256===hash(readFileSync(paths[index]))));
}
assert.deepEqual(current.results.map(row=>row.id),prior.results.map(row=>row.id));
const rows=[];
for(const row of current.results){
  const old=prior.results.find(item=>item.id===row.id);assert(!row.failure&&!old.failure);
  assert.deepEqual(row.stages.map(stage=>stage.id),old.stages.map(stage=>stage.id));
  for(const [index,stage]of row.stages.entries()){
    const before=old.stages[index];
    for(const key of ['role','sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','values','feedback','errors'])assert.deepEqual(stage[key],before[key],row.id+':'+stage.id+':'+key);
    assert.deepEqual(stage.publishedTypingErrors,[]);
    assert(stage.events.every(event=>Number.isSafeInteger(event.occurrences)&&event.occurrences>0));
    if(mode!=='updates'){
      const target=/-target$/.test(row.id),capture=/-capture$/.test(row.id),distinct=/-distinct-(64|65)-/.exec(row.id),repeated=/-(repeated|child)-(30|64|70|200)-/.exec(row.id);
      if(capture||/-untrack-control$/.test(row.id)){assert.equal(stage.events.length,0);assert.equal(before.events.length,0);}
      if(target&&distinct){
        const count=Number(distinct[1]),expected=mode==='registered'&&count>64?0:count;
        assert.equal(stage.events.length,expected);assert.equal(before.events.length,expected);
        assert.equal(new Set(stage.events.map(event=>event.identity.id)).size,expected);
        assert(stage.events.every(event=>event.occurrences===1));
        if(!expected)assert(stage.continuationGaps.some(gap=>/budget/.test(gap.reason)));
      }else if(target&&repeated){
        const count=Number(repeated[2]);assert.equal(stage.events.length,1);assert.equal(stage.events[0].occurrences,count);
        assert.equal(stage.values.calls,count);
        assert.equal(before.events.length,mode==='registered'&&count>64?0:1);
        if(before.events.length)assert.equal(before.events[0].occurrences,count);
        if(mode==='registered')assert.equal(stage.events[0].asyncContinuation.chain.length,repeated[1]==='child'?2:1);
      }else if(/-constant-control$/.test(row.id)){
        assert.equal(stage.events.length,1);assert.equal(stage.events[0].occurrences,70);
        assert.equal(before.events.length,mode==='registered'?0:1);
      }else if(/-terminal-control$/.test(row.id)){
        assert.equal(stage.events.length,mode==='registered'?0:1);
        assert.equal(before.events.length,mode==='registered'?0:1);
      }
      for(const event of stage.events){
        assert.equal(!!event.callbackRegistration,mode==='registered');
        assert.equal(!!event.asyncContinuation,mode==='registered');
      }
    }
    rows.push({consumer:row.id,stage:stage.id,currentEvents:stage.events.length,priorEvents:before.events.length,occurrences:stage.events.reduce((sum,event)=>sum+event.occurrences,0),currentHints:stage.current.notes.length,priorHints:before.current.notes.length});
  }
  if(mode==='updates'){
    const next=row.instrumentation.simulatedConfigEvents,previous=old.instrumentation.simulatedConfigEvents;
    assert.equal(next.length,row.stages.length);assert.equal(previous.length,row.stages.length);
    for(const event of next)assert.deepEqual(event.before,event.after);
    for(const event of previous){assert.equal(event.before.inputSha256,event.after.inputSha256);assert(event.after.generation>event.before.generation);assert(event.after.invalidation>event.before.invalidation);}
    for(const change of row.instrumentation.cacheInvalidations)assert.equal(change.inputChange.valid,false);
    for(let index=1;index<row.stages.length;index++)assert.notDeepEqual(row.stages[index].revision,row.stages[index-1].revision);
  }
}
const report={authority:false,certification:false,mode,finishedAt:new Date().toISOString(),scope:'count and coherent revision assertions after independent published-typing/source/frame/plain-behavior audits; observed intent and whole package result flow remain open',validator:{path:new URL(import.meta.url).pathname,sha256:hash(readFileSync(new URL(import.meta.url)))},inputs:paths.map(path=>({path,sha256:hash(readFileSync(path))})),current:currentAudit.summary,prior:priorAudit.summary,comparisons:rows};
writeFileSync(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({mode,comparisons:rows.length,current:currentAudit.summary,prior:priorAudit.summary}));
