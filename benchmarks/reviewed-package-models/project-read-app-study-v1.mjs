// Measured project-aware admission on unchanged retained real source files.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash,packageDigest,read} from './catalog.mjs';
import {ts} from './lower.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformProjectReads} from './async-read-transform-v3.mjs';
import {readProgram} from './async-read-transform-v2.mjs';

const [inventoryArg,outputArg]=process.argv.slice(2),inventoryPath=resolve(inventoryArg),output=resolve(outputArg),inventory=read(inventoryPath);
assert(!existsSync(output));
function authenticate(){for(const pin of inventory.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);for(const runtime of inventory.runtimePins)assert.equal(packageDigest(runtime.path),runtime.digest,runtime.path);}
authenticate();
const profile=['project-read-app-study-v1.mjs','project-read-session-v1.mjs','async-read-transform-v3.mjs','async-read-sites-v2.mjs','async-read-transform-v2.mjs','catalog.mjs','lower.mjs'].map(name=>{
  const path=new URL(name,import.meta.url).pathname;return {path,sha256:hash(readFileSync(path))};});
const report={authority:false,certification:false,startedAt:new Date().toISOString(),inventory:{path:inventoryPath,sha256:hash(readFileSync(inventoryPath))},profile,projects:[]};
for(const original of inventory.projects){
  if(original.refused)continue;
  const session=projectReadSession(original.root,{configPath:join(original.root,original.config.split('/').at(-1))});
  const selected=new Map(original.currentPerFileProbes.map(row=>[row.path,original.files.find(file=>file.path===row.path)]));
  for(const file of original.files)if(file.sites.length)selected.set(file.path,file);
  const row={app:original.app,files:[],originalProjectErrors:original.errors.map(error=>error.code)};report.projects.push(row);
  for(const file of selected.values()){
    const text=readFileSync(file.path,'utf8');assert.equal(hash(text),file.sha256);
    let old=original.currentPerFileProbes.find(probe=>probe.path===file.path);
    if(!old){const start=performance.now(),program=readProgram(text,file.path);old={totalMs:performance.now()-start,typingErrors:ts.getPreEmitDiagnostics(program).filter(error=>error.category===ts.DiagnosticCategory.Error)};}
    const start=performance.now(),transformed=transformProjectReads(text,file.path,session),totalMs=performance.now()-start;
    const errors=session.get(file.path,text).errors;
    assert.deepEqual(errors.map(error=>error.code),row.originalProjectErrors);
    if(!errors.length)assert.deepEqual(transformed.sites,file.sites);
    else assert.equal(transformed.sites.length,0);
    row.files.push({path:file.path,sourceSha256:file.sha256,currentPerFileErrors:old.typingErrors.map(error=>error.code),currentPerFileMs:old.totalMs,
      projectErrors:errors.map(error=>error.code),candidateSites:transformed.sites.length,totalMs,session:transformed.session,open:transformed.open});
  }
  row.sessionStats={...session.stats};
  writeFileSync(output,JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({app:row.app,files:row.files.length,builds:row.sessionStats.builds,candidates:row.files.reduce((sum,file)=>sum+file.candidateSites,0),validationMs:row.sessionStats.validationMs}));
}
authenticate();for(const pin of profile)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);
report.finishedAt=new Date().toISOString();const files=report.projects.flatMap(row=>row.files);
report.summary={projects:report.projects.length,files:files.length,projectBuilds:report.projects.reduce((sum,row)=>sum+row.sessionStats.builds,0),
  falsePerFileTypingExclusionsCorrected:files.filter(file=>file.currentPerFileErrors.length&&!file.projectErrors.length).length,
  realProjectTypingExclusionsPreserved:files.filter(file=>file.projectErrors.length).length,
  admittedSites:files.reduce((sum,file)=>sum+file.candidateSites,0),
  claim:'source/type admission and local cost; no package-defect detection or coverage estimate'};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));
