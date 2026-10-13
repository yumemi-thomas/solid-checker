// Preserve successful measured observations after a later control/audit failed.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {execute} from './process.mjs';
import {directory,historical} from './profile.mjs';
import {hash,read} from '../catalog.mjs';
import {authenticate,authenticateObservation} from './evidence.mjs';
const [parentArg,outArg]=process.argv.slice(2),parentPath=resolve(parentArg),parent=read(parentPath),out=resolve(outArg);
assert(parent.failure&&parent.runs.length===parent.config.order.length&&!existsSync(out));
for(const pin of parent.inputs)authenticate(pin);
for(const row of parent.runs){assert.equal(row.code,0);authenticateObservation(row.result);}
mkdirSync(out,{recursive:true});
const config={...parent.config},report={authority:false,certification:false,developmentOnly:true,startedAt:new Date().toISOString(),config,
  recoveredFrom:{path:parentPath,sha256:hash(readFileSync(parentPath)),failure:parent.failure},runs:parent.runs.map(row=>({...row})),inputs:[...parent.inputs,{path:parentPath,sha256:hash(readFileSync(parentPath))}]};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');save();
try{
  config.freeze=join(out,'working-freeze.json');const freeze=await execute([join(directory,'freeze.mjs'),config.freeze],join(out,'freeze.log'));assert.equal(freeze.code,0);
  const root=join(out,'plain');report.plain=await execute(['--expose-gc',join(directory,'browser.mjs'),config.cases,config.freeze,root,config.binaries.chromium,'plain',join(resolve(parentPath,'..'),'selection.json'),'chromium'],join(out,'plain.log'));
  save();assert.equal(report.plain.code,0,report.plain.log);const baseline=join(root,'results.json');report.baseline={path:baseline,sha256:hash(readFileSync(baseline))};
  for(const [index,row]of report.runs.entries()){
    const path=join(out,`${index+1}-${row.engine}-audit.json`);
    row.auditExecution=await execute(['--expose-gc',join(historical,'feedback-revision-audit-v19.mjs'),config.cases,row.result.path,baseline,path],join(out,`${index+1}-${row.engine}-audit.log`));save();assert.equal(row.auditExecution.code,0,row.auditExecution.log);
    row.audit={path,sha256:hash(readFileSync(path))};save();console.log(JSON.stringify({engine:row.engine,auditedStages:read(path).summary.stages}));
  }
  report.inputs.push({path:config.freeze,sha256:hash(readFileSync(config.freeze))},{path:new URL(import.meta.url).pathname,sha256:hash(readFileSync(new URL(import.meta.url)))});
  for(const pin of report.inputs)authenticate(pin);report.finishedAt=new Date().toISOString();save();
}catch(error){report.failure={message:error.message};save();throw error;}
