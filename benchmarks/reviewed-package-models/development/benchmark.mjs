import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {execute} from './process.mjs';
import {directory,historical} from './profile.mjs';
import {hash,read} from '../catalog.mjs';
const [configArg,outArg]=process.argv.slice(2),config=read(resolve(configArg)),out=resolve(outArg);assert(!existsSync(out));
assert(config.hypothesis?.trim()&&config.success?.trim());mkdirSync(out,{recursive:true});
const selection=join(out,'selection.json');writeFileSync(selection,JSON.stringify({caseIds:config.caseIds})+'\n');
const report={authority:false,certification:false,developmentOnly:true,startedAt:new Date().toISOString(),config,runs:[],
  inputs:[resolve(configArg),config.cases,config.freeze,selection,new URL(import.meta.url).pathname,...Object.values(config.binaries)].map(path=>({path,sha256:hash(readFileSync(path))}))};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');save();
try{
  for(const [index,engine]of config.order.entries()){
    const root=join(out,`${index+1}-${engine}`),measured=await execute(['--expose-gc',join(directory,'browser.mjs'),config.cases,config.freeze,root,config.binaries[engine],'reads',selection,engine],join(out,`${index+1}-${engine}.log`),{memory:true});
    const row={engine,root,...measured};report.runs.push(row);save();assert.equal(row.code,0,row.log);
    const result=read(join(root,'results.json'));assert(result.finishedAt);row.timings=result.timings;
    row.result={path:join(root,'results.json'),sha256:hash(readFileSync(join(root,'results.json')))};save();
    console.log(JSON.stringify({engine,wallMs:row.wallMs,memory:{browser:row.memory.maxBrowserRssBytes,total:row.memory.maxTotalRssBytes}}));
  }
  // Audits and plain execution happen after timed comparisons.
  let baseline=config.baseline;
  if(!baseline){
    const root=join(out,'plain');report.plain=await execute(['--expose-gc',join(directory,'browser.mjs'),config.cases,config.freeze,root,config.binaries.chromium,'plain',selection,'chromium'],join(out,'plain.log'));
    save();assert.equal(report.plain.code,0,report.plain.log);baseline=join(root,'results.json');
  }
  report.baseline={path:baseline,sha256:hash(readFileSync(baseline))};
  for(const [index,row]of report.runs.entries()){
    const path=join(out,`${index+1}-${row.engine}-audit.json`);
    row.auditExecution=await execute(['--expose-gc',join(historical,'feedback-revision-audit-v19.mjs'),config.cases,row.result.path,baseline,path],join(out,`${index+1}-${row.engine}-audit.log`));save();assert.equal(row.auditExecution.code,0,row.auditExecution.log);
    row.audit={path,sha256:hash(readFileSync(path))};save();
  }
  for(const pin of report.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  report.finishedAt=new Date().toISOString();save();
}catch(error){report.failure={message:error.message};save();throw error;}
