// Full eligible-source admission and cost; no inferred runtime defects.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash,packageDigest,read} from './catalog.mjs';
import {projectReadSession} from './project-read-session-v1.mjs';
import {transformCandidateProjectReads} from './async-read-transform-v4.mjs';

const [inventoryArg,outputArg]=process.argv.slice(2),inventoryPath=resolve(inventoryArg),out=resolve(outputArg),inventory=read(inventoryPath);
assert(!existsSync(out));
function authenticate(){for(const pin of inventory.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);for(const runtime of inventory.runtimePins)assert.equal(packageDigest(runtime.path),runtime.digest,runtime.path);}
authenticate();
const profile=['project-read-app-study-v2.mjs','project-read-session-v1.mjs','async-read-transform-v4.mjs','async-read-transform-v3.mjs','async-read-prefilter-v1.mjs','async-read-sites-v2.mjs','lower.mjs','catalog.mjs']
 .map(name=>{const path=new URL(name,import.meta.url).pathname;return {path,sha256:hash(readFileSync(path))};});
const report={authority:false,certification:false,startedAt:new Date().toISOString(),inventory:{path:inventoryPath,sha256:hash(readFileSync(inventoryPath))},profile,projects:[]};
for(const original of inventory.projects){if(original.refused)continue;
 const session=projectReadSession(original.root,{configPath:join(original.root,original.config.split('/').at(-1))}),row={app:original.app,files:[],projectTypingErrors:original.errors.length};report.projects.push(row);
 for(const file of original.files.filter(file=>file.currentPluginEligible)){
  const text=readFileSync(file.path,'utf8');assert.equal(hash(text),file.sha256);const start=performance.now(),result=transformCandidateProjectReads(text,file.path,session),totalMs=performance.now()-start;
  if(result.session.skipped)assert.equal(file.sites.length,0,'Syntax gate cannot discard an enrolled read');
  else if(original.errors.length){assert.equal(result.sites.length,0);assert.deepEqual(result.open.find(item=>item.reason==='TypeScript owns this input').codes,original.errors.map(error=>error.code));}
  else assert.deepEqual(result.sites,file.sites);
  row.files.push({path:file.path,sourceSha256:file.sha256,totalMs,session:result.session,enrolledSites:result.sites.length,sites:result.sites});
 }
 row.sessionStats={...session.stats};writeFileSync(out,JSON.stringify(report,null,2)+'\n');
 console.log(JSON.stringify({app:row.app,files:row.files.length,skipped:row.files.filter(file=>file.session.skipped).length,builds:row.sessionStats.builds,enrolled:row.files.reduce((sum,file)=>sum+file.enrolledSites,0),ms:row.files.reduce((sum,file)=>sum+file.totalMs,0)}));
}
authenticate();for(const pin of profile)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);
const files=report.projects.flatMap(row=>row.files);report.finishedAt=new Date().toISOString();
report.summary={projects:report.projects.length,files:files.length,syntaxSkipped:files.filter(file=>file.session.skipped).length,projectBuilds:report.projects.reduce((sum,row)=>sum+row.sessionStats.builds,0),
 enrolledSites:files.reduce((sum,file)=>sum+file.enrolledSites,0),transformationMs:files.reduce((sum,file)=>sum+file.totalMs,0),
 claim:'exact source admission and measured transformation cost only; no runtime read context or app-defect verdict'};
writeFileSync(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));
