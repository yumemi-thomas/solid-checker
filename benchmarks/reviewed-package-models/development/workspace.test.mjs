import test from 'node:test';
import assert from 'node:assert/strict';
import {read} from '../catalog.mjs';
import {replayWorkspace} from './workspace.mjs';
import {nativeReadFeedback as original} from '../native-read-feedback-v28.mjs';
const configPath=process.env.SOLID_EXPERIMENT_REPLAY_CONFIG;
assert(configPath,'Set SOLID_EXPERIMENT_REPLAY_CONFIG to an authenticated replay configuration');
test('a distinct selector reuses the program and leaves recorded execution intact',()=>{
  const config=read(configPath),workspace=replayWorkspace(config.observation),row=workspace.report.results.find(row=>row.stages.at(-1).current.notes.length);
  assert(row);const recorded=JSON.stringify(workspace.report),first=workspace.project(row.id,original);
  const edited=(...args)=>{const projected=original(...args);return {...projected,notes:projected.notes.map(note=>({...note,message:note.message+' [offline wording probe]'}))};};
  const second=workspace.project(row.id,edited);
  assert(!first.reusedProgram&&second.reusedProgram);assert.equal(workspace.stats.builds,1);assert.equal(workspace.stats.hits,1);
  assert.equal(second.current.notes[0].message,first.current.notes[0].message+' [offline wording probe]');
  assert.deepEqual(second.current.notes.map(note=>note.witness),first.current.notes.map(note=>note.witness));
  assert.equal(JSON.stringify(workspace.report),recorded);assert.deepEqual(first.current,row.stages.at(-1).current);workspace.close();
  assert.throws(()=>workspace.project(row.id,original),/closed/);
});
