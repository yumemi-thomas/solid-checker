import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {authenticate,authenticateObservation,authenticateStage} from './evidence.mjs';
import {programPool} from './program-pool.mjs';
import {projectTransaction} from './transaction.mjs';
import {packageSourceSession} from '../package-source-session-v2.mjs';
import {closurePins,packageRoot,hash} from '../catalog.mjs';
import {historical} from './profile.mjs';

// A resident workspace can re-project a case after selector edits without
// rebuilding its unchanged TS program. The two-program default bounds memory.
export function replayWorkspace(observation,{capacity=2}={}){
  const report=authenticateObservation(observation),pool=programPool({capacity});let closed=false;
  const producerIdentity={node:process.version,typescript:closurePins(packageRoot(resolve('packages/cli'),'typescript')),
    sourceProducer:['package-source-session-v2.mjs','project-read-session-v2.mjs','lower.mjs'].map(name=>{const path=join(historical,name);return {path,sha256:hash(readFileSync(path))};})};
  return {report,producerIdentity,stats:pool.stats,project(id,projector){
    assert(!closed,'Replay workspace is closed');
    authenticate(observation);for(const pin of producerIdentity.sourceProducer)authenticate(pin);
    const row=report.results.find(row=>row.id===id);assert(row&&!row.failure);assert.deepEqual(row.pageErrors,[]);
    const started=performance.now(),stage=row.stages.at(-1),inputs=authenticateStage(stage),root=join(resolve(observation.path,'..'),row.id),path=join(root,'src/main.tsx'),code=readFileSync(path,'utf8');
    assert.equal(hash(code),stage.sourceSha256);
    const cached=pool.get(producerIdentity,inputs,()=>{
      const session=packageSourceSession(root),state=session.get(path,code);
      assert.equal(hash(JSON.stringify(session.inputs())),stage.revision.inputSha256,'Rebuilt program has different source/resolution inputs');
      assert.deepEqual(session.runtimeSources(),stage.runtimeSources);
      return {get(requested,text){const actual=session.get(requested,text);return {...actual,revision:stage.revision};},inputs:()=>session.inputs(),
        acceptRevision(revision){return JSON.stringify(revision)===JSON.stringify(stage.revision)?session.acceptRevision(state.revision):{valid:false,reason:'observation has no issued current project revision'};}};
    });
    const projected=projectTransaction(cached.value,path,code,facade=>projector(facade,path,code,stage.events,stage.identityStats));
    assert.equal(projected.result.acceptedEvents,stage.events.length);
    authenticateStage(stage);
    return {id:row.id,stage:stage.id,observedRevision:stage.revision,reusedProgram:cached.reused,durationMs:performance.now()-started,transaction:projected.metrics,current:projected.result};
  },close(){if(closed)return;try{authenticateObservation(observation);}finally{pool.clear();closed=true;}}};
}
