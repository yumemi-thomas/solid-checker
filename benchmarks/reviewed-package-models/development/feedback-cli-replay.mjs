// Bridge authenticated historical observations into the reusable CLI boundary.
// It does not relabel old browser execution as execution of the RC.13 compiler.
import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,mkdirSync,existsSync,realpathSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {authenticateObservation,authenticateStage} from './evidence.mjs';
import {hash} from '../catalog.mjs';
import {feedbackSnapshot,captureTemplate,inspectDevelopmentFeedback} from '../../../packages/cli/scripts/development-feedback.mjs';

const [inputArg,outputArg]=process.argv.slice(2),input=resolve(inputArg),output=resolve(outputArg);
assert(!existsSync(output));mkdirSync(output,{recursive:true});
const pin={path:input,sha256:hash(readFileSync(input))},report=authenticateObservation(pin);
const implementationInputs=[new URL('./feedback-cli-replay.mjs',import.meta.url).pathname,
  new URL('../../../packages/cli/scripts/development-feedback.mjs',import.meta.url).pathname,
  new URL('../../../packages/cli/bin/launcher.mjs',import.meta.url).pathname,
  process.env.SOLID_CHECKER_NATIVE_BIN,process.env.SOLID_TYPEFACTS_BIN].map(path=>{
    assert(path,'Replay requires explicit checker and Type Facts binaries');
    path=realpathSync(path);return {path,sha256:hash(readFileSync(path))};
  });
const validateImplementation=()=>{for(const pin of implementationInputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256,'Feedback implementation changed during replay');};
const results=[];
for(const row of report.results){
  assert(!row.failure);const stage=row.stages.at(-1);authenticateStage(stage);
  const root=join(dirname(input),row.id),project=join(root,'tsconfig.json');
  const snapshot=feedbackSnapshot(project),capture=captureTemplate(snapshot),runtime=new Map();
  for(const [index,event] of stage.events.entries()){
    if(event.context?.observer!==false||event.context?.owner!==false)continue;
    const site=event.site,path=site.path,text=readFileSync(path,'utf8');
    assert.equal(hash(text),site.sourceSha256);assert.equal(hash(text),stage.sourceSha256);
    assert(event.originalFrames.some(frame=>frame?.path===path&&frame.sourceSha256===site.sourceSha256));
    const premise=event.nativeRead.premise,runtimePath=realpathSync(premise.path);
    assert.equal(runtimePath,premise.path);assert.equal(hash(readFileSync(runtimePath)),premise.sourceSha256);
    runtime.set(runtimePath,{path:runtimePath,sha256:premise.sourceSha256});
    capture.events.push({id:`${row.id}:${stage.id}:${index}`,kind:'untracked-read',tracking:'untracked',
      runtimeInput:runtimePath,message:'The recorded application execution read reactive state while tracking was off. Reactive intent remains open.',
      sourceSha256:site.sourceSha256,location:{path,startByte:Buffer.byteLength(text.slice(0,site.start)),endByte:Buffer.byteLength(text.slice(0,site.end))}});
  }
  capture.runtimeInputs=[...runtime.values()];capture.origin={observation:pin,stage:stage.id,revision:stage.revision};
  const dir=join(output,row.id);mkdirSync(dir);writeFileSync(join(dir,'capture.json'),JSON.stringify(capture,null,2)+'\n');
  const feedback=inspectDevelopmentFeedback(project,{capture});authenticateStage(stage);validateImplementation();
  writeFileSync(join(dir,'feedback.json'),JSON.stringify(feedback,null,2)+'\n');
  results.push({id:row.id,typingErrors:feedback.typingErrorCount,nativeFindings:feedback.analysis.findings.length,
    nativeViolations:feedback.analysis.findings.filter(f=>f.kind==='violation').length,
    runtimeEvents:capture.events.length,observations:feedback.observations.length,durationMs:feedback.durationMs,
    historicalHintCount:stage.current.notes.length,excluded:feedback.excluded});
  console.log(JSON.stringify(results.at(-1)));
}
const summary={applications:results.length,typingErrors:results.reduce((n,r)=>n+r.typingErrors,0),
  nativeFindings:results.reduce((n,r)=>n+r.nativeFindings,0),observations:results.reduce((n,r)=>n+r.observations,0)};
writeFileSync(join(output,'results.json'),JSON.stringify({authority:false,certification:false,observation:pin,
  implementationInputs,analysisCompiler:'local RC.13 candidate; original browser execution retains RC.9',summary,results},null,2)+'\n');
