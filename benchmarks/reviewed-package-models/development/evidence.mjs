import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {directory} from './profile.mjs';
import {hash,read,closurePins} from '../catalog.mjs';
import {validateProjectInputs} from '../project-read-session-v2.mjs';
export function authenticate(pin){assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);}
export function authenticateObservation(pin){
  authenticate(pin);const report=read(pin.path);
  assert(report.finishedAt&&!report.failure&&report.variant==='reads');
  assert.equal(report.authority,false);assert.equal(report.certification,false);
  for(const input of report.inputs.files)authenticate(input);
  const sealPin=report.inputs.files.find(row=>row.path.endsWith('.json')&&row.path.includes('freeze'));
  assert(sealPin,'Observed profile is missing');const seal=read(sealPin.path);
  for(const input of [...seal.files,seal.baseline])authenticate(input);
  for(const input of seal.workingFiles??[]){
    assert(seal.files.some(pin=>pin.path===input.archivePath&&pin.sha256===input.sha256),'Original working code lacks an archived byte identity');
  }
  for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
  return report;
}
export function activePins(){return readdirSync(directory).filter(name=>name.endsWith('.mjs')).sort().map(name=>{const path=join(directory,name);return {path,sha256:hash(readFileSync(path))};});}
export function authenticateStage(stage){
  assert.equal(hash(JSON.stringify(stage.inputManifest)),stage.revision.inputSha256);
  // JSON represents omitted optional directory arguments as null.
  const inputs=stage.inputManifest.map(input=>input.kind==='listing'?{...input,arguments:input.arguments.map(value=>value===null?undefined:value)}:input);
  assert(validateProjectInputs(inputs).valid,'Saved observation source/resolution inputs changed');
  for(const event of stage.events)assert.deepEqual(event.site.projectRevision,stage.revision);
  return inputs;
}
