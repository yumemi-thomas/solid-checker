// Derived projection of recorded execution. Never labels new code as executed.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {basename,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {performance} from 'node:perf_hooks';
import {authenticate,activePins} from './evidence.mjs';
import {replayWorkspace} from './workspace.mjs';
import {nativeReadFeedback as originalProject} from '../native-read-feedback-v28.mjs';
import {hash,read} from '../catalog.mjs';
import {directory} from './profile.mjs';

const [configArg,outArg]=process.argv.slice(2),configPath=resolve(configArg),config=read(configPath),out=resolve(outArg);
assert(!existsSync(out));assert(config.hypothesis?.trim()&&config.success?.trim());
const started=performance.now(),selector=resolve(config.selector??join(directory,'selector.mjs'));
assert.equal(selector,join(directory,'selector.mjs'),'Edit the active selector seam; arbitrary external selector imports are unsupported');
const workingPins=activePins(),workspace=replayWorkspace(config.observation),implementation=await import(pathToFileURL(selector));
assert.equal(typeof implementation.project,'function');
mkdirSync(join(out,'sources'),{recursive:true});
const archived=workingPins.map(pin=>{
  const path=join(out,'sources',basename(pin.path));writeFileSync(path,readFileSync(pin.path));
  return {path,sha256:pin.sha256,executedPath:pin.path};
});
const selectorPin=archived.find(pin=>pin.executedPath===selector),results=[];
for(const row of workspace.report.results){
  const original=workspace.project(row.id,originalProject);
  assert.deepEqual(original.current,row.stages.at(-1).current,'Rebuilt projection differs from recorded execution');
  const current=workspace.project(row.id,implementation.project);
  if(config.requireOriginalEquality)assert.deepEqual(current.current,original.current,'Selector differs from original projection');
  for(const note of current.current.notes){assert.equal(note.authority,false);assert.equal(note.certification,false);assert.equal(note.severity,'info');}
  results.push({id:row.id,stage:current.stage,observedRevision:current.observedRevision,projections:[{...original,role:'original-selector'},{...current,role:'working-selector'}]});
}
workspace.close();for(const pin of workingPins)authenticate(pin);
const result={authority:false,certification:false,developmentOnly:true,derived:true,execution:'saved observations; no browser run',hypothesis:config.hypothesis,success:config.success,
  observedProfile:workspace.report.inputs,observation:config.observation,selector:selectorPin,workingSources:archived,config:{path:configPath,sha256:hash(readFileSync(configPath))},producerIdentity:workspace.producerIdentity,
  scope:'Only final stages whose complete source/resolution inputs still match disk; selector changes need focused tests and an independent audit',pool:workspace.stats,results,
  finishedAt:new Date().toISOString(),durationMs:performance.now()-started};
writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({cases:results.length,durationMs:result.durationMs,pool:workspace.stats}));
