import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {validateProjectInputs} from '../project-read-session-v2.mjs';
import {hash} from '../catalog.mjs';

// Reuse the exact current program in one synchronous projection only. Never
// reuse observations across edits or leave the facade usable afterwards.
export function projectTransaction(session,path,code,project,validate=validateProjectInputs){
  const started=performance.now(),state=session.get(path,code),inputs=session.inputs();
  assert.equal(hash(JSON.stringify(inputs)),state.revision.inputSha256);
  assert(validate(inputs).valid,'Inputs changed before projection');
  assert(session.acceptRevision(state.revision).valid,'Revision retired before projection');
  let active=true,lookups=0,revisionChecks=0;
  const facade={...session,get(requested,text){
    assert(active,'Projection transaction is closed');lookups++;
    const source=state.program.getSourceFile(resolve(requested));
    assert(source,'Source is outside this program');assert.equal(source.text,text);
    assert.equal(readFileSync(requested,'utf8'),text);
    return {...state,source,reused:true,buildMs:0};
  },acceptRevision(revision){
    assert(active,'Projection transaction is closed');revisionChecks++;
    return JSON.stringify(revision)===JSON.stringify(state.revision)?{valid:true}:{valid:false,reason:'observation has no issued current project revision'};
  },invalidate(){throw Error('Cannot invalidate within a projection transaction');}};
  try{
    const result=project(facade);
    assert(!result||typeof result.then!=='function','Projection must finish synchronously');
    assert(validate(inputs).valid,'Inputs changed during projection');
    assert(session.acceptRevision(state.revision).valid,'Revision retired during projection');
    assert.deepEqual(session.inputs(),inputs,'Input manifest changed during projection');
    return {result,metrics:{lookups,revisionChecks,durationMs:performance.now()-started,inputCount:inputs.length}};
  }finally{active=false;}
}
