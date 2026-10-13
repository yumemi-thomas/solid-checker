// Sequential paired browser pilot. Small sample; no general browser speed claim.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {existsSync,mkdirSync,openSync,closeSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {hash,read} from './catalog.mjs';

const out=resolve(process.argv[2]);assert(!existsSync(out));mkdirSync(out,{recursive:true});
const directory=new URL('.',import.meta.url).pathname,cases=join(directory,'noise-zero-replay-cases-v1.mjs'),
  freeze=resolve('rust/target/lightpanda-detector-freeze-v1.json'),selection=resolve('rust/target/development-speed-focus-v1/selection.json'),
  baseline=resolve('rust/target/noise-zero-replay-plain-v1/results.json'),
  binaries={lightpanda:resolve('rust/target/lightpanda-1.0.0/lightpanda'),chromium:'/Users/thomas/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing'};
const report={authority:false,certification:false,developmentOnly:true,startedAt:new Date().toISOString(),order:['chromium','lightpanda','lightpanda','chromium'],runs:[],inputs:[cases,freeze,selection,baseline,...Object.values(binaries),new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))}))};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(report,null,2)+'\n');save();
async function execute(args,log){
  const fd=openSync(log,'wx'),start=performance.now();let code;
  try{code=await new Promise((yes,no)=>{const child=spawn(process.execPath,args,{stdio:['ignore',fd,fd]});child.once('error',no);child.once('exit',yes);});}
  finally{closeSync(fd);}
  return {code,wallMs:performance.now()-start};
}
try{
  // No audits or other benchmark workloads run while browser time is measured.
  for(const [index,engine]of report.order.entries()){
    const root=join(out,`${index+1}-${engine}`),log=join(out,`${index+1}-${engine}.log`);
    const measured=await execute(['--expose-gc',join(directory,'feedback-revision-browser-v33.mjs'),cases,freeze,root,binaries[engine],'reads',selection,engine],log);
    const row={engine,root,log,...measured};report.runs.push(row);save();assert.equal(measured.code,0,log);
    const result=read(join(root,'results.json'));assert(result.finishedAt&&result.engine.kind===engine);
    row.timings=result.timings;row.result={path:join(root,'results.json'),sha256:hash(readFileSync(join(root,'results.json')))};save();
    console.log(JSON.stringify({engine,wallSeconds:row.wallMs/1000}));
  }
  for(const [index,row]of report.runs.entries()){
    const path=join(out,`${index+1}-${row.engine}-audit.json`),log=join(out,`${index+1}-${row.engine}-audit.log`);
    const measured=await execute(['--expose-gc',join(directory,'feedback-revision-audit-v19.mjs'),cases,row.result.path,baseline,path],log);
    row.auditExecution={...measured,log};save();assert.equal(measured.code,0,log);
    row.audit={path,sha256:hash(readFileSync(path))};save();
  }
  for(const pin of report.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);
  report.finishedAt=new Date().toISOString();save();console.log(JSON.stringify({runs:report.runs.length,audits:report.runs.filter(row=>row.audit).length}));
}catch(error){report.failure={message:error.message};save();throw error;}
