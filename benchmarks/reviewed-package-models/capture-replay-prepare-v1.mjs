// Reconstruct public source programs and prepare an isolated proposal population.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {ts} from './lower.mjs';
import {hash,read,closurePins} from './catalog.mjs';
import {captureReplayPlan,applyCaptureReplayPlan} from './capture-replay-plan-v1.mjs';
const [caseArg,reportArg,outArg]=process.argv.slice(2),casePath=resolve(caseArg),reportPath=resolve(reportArg),out=resolve(outArg);assert(!existsSync(out));
const cases=(await import(pathToFileURL(casePath))).default,report=read(reportPath);assert(report.finishedAt&&report.variant==='reads');
for(const pin of report.inputs.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);assert(report.inputs.files.some(pin=>pin.path===casePath));for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
const seal=read(report.inputs.files.find(pin=>pin.path!==casePath).path);for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const rows=[];
for(const row of report.results){
  const challenge=cases.find(c=>c.id===row.id);assert(challenge&&row.stages.length===1);const stage=row.stages[0],root=join(dirname(reportPath),row.id),path=join(root,'src/main.tsx'),config=join(root,'tsconfig.json');assert.equal(hash(readFileSync(path)),stage.sourceSha256);assert.deepEqual(stage.publishedTypingErrors,[]);
  const parsed=ts.getParsedCommandLineOfConfigFile(config,{}, {...ts.sys,onUnRecoverableConfigFileDiagnostic:d=>{throw Error(ts.flattenDiagnosticMessageText(d.messageText,'\n'));}}),program=ts.createProgram(parsed.fileNames,parsed.options);assert.deepEqual(ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error),[]);
  const plans=[];for(const event of stage.events){const result=captureReplayPlan(program,event);plans.push({...result,nativeIdentity:event.identity.id});}
  const admitted=plans.filter(x=>x.plan);assert(admitted.length<=1,'multiple replay edits require another profile');
  rows.push({id:row.id,originalRole:stage.role,originalValue:stage.afterUpdate,desired:stage.desired,observations:stage.events.length,originalHints:stage.current.notes.length,plans,...(admitted.length?{repaired:{...challenge,id:'capture-replay-'+row.id,source:applyCaptureReplayPlan(challenge.source,admitted[0].plan),artifactOrigin:'automatically-generated-source-capture-proposal-for-isolated-replay',stages:[{...challenge.stages[0],role:'control',afterUpdate:stage.desired,desired:stage.desired}]}}:{})});
  await new Promise(resolve=>setImmediate(resolve));globalThis.gc?.();
}
writeFileSync(out,JSON.stringify({authority:false,certification:false,scope:'source-bound proposals for explicit isolated replay; getter timing and side effects change, repair safety and intent remain unproved',inputs:[casePath,reportPath].map(path=>({path,sha256:hash(readFileSync(path))})),planner:{path:new URL('./capture-replay-plan-v1.mjs',import.meta.url).pathname,sha256:hash(readFileSync(new URL('./capture-replay-plan-v1.mjs',import.meta.url)))},rows},null,2)+'\n');
console.log(JSON.stringify({consumers:rows.length,proposals:rows.filter(row=>row.repaired).length,open:rows.filter(row=>row.plans.some(item=>item.open)).map(row=>({id:row.id,reasons:row.plans.filter(item=>item.open).map(item=>item.open.reason)}))}));
