// A focused development run; full/fresh population gates remain separate.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {spawn} from 'node:child_process';
import {performance} from 'node:perf_hooks';
import {hash,read} from './catalog.mjs';

const [outArg,browserArg,configArg]=process.argv.slice(2),directory=new URL('.',import.meta.url).pathname;
const defaults={
  cases:join(directory,'noise-zero-replay-cases-v1.mjs'),
  freeze:resolve('rust/target/development-speed-detector-freeze-v1.json'),
  baseline:resolve('rust/target/noise-zero-replay-plain-v1/results.json'),
  caseIds:['serial','concurrent'].flatMap(mode=>[
    `async-body-return-${mode}-adopted-child-object-deferred`,
    `async-body-return-${mode}-reaction-read-deferred`,
    `warning-accuracy-fresh-${mode}-named-allocation-open`,
    `warning-accuracy-fresh-${mode}-identity-control-open`,
  ]),
  tests:['bounded-body-return-v2.test.mjs','late-lexical-continuation-v1.test.mjs','lexical-entry-map-v1.test.mjs','development-case-selection-v1.test.mjs'].map(name=>join(directory,name)),
};
const config=configArg?read(resolve(configArg)):defaults;
assert(config.cases&&config.freeze&&Array.isArray(config.caseIds)&&config.caseIds.length&&Array.isArray(config.tests)&&config.tests.length);
const plan={developmentOnly:true,cases:resolve(config.cases),freeze:resolve(config.freeze),baseline:config.baseline?resolve(config.baseline):null,caseIds:config.caseIds,tests:config.tests.map(path=>resolve(path)),steps:['focused tests','selected observed browser run',...(config.baseline?[]:['selected plain browser run']),'independent selected-case audit']};
if(outArg==='--plan'){console.log(JSON.stringify(plan,null,2));process.exit(0);}
assert(outArg&&browserArg,'usage: OUT BROWSER [CONFIG] or --plan');
const out=resolve(outArg);assert(!existsSync(out),'use a fresh output directory');mkdirSync(out,{recursive:true});
const selection=join(out,'selection.json');writeFileSync(selection,JSON.stringify({caseIds:plan.caseIds})+'\n');
const result={authority:false,certification:false,developmentOnly:true,startedAt:new Date().toISOString(),plan,phases:[]},started=performance.now();
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2)+'\n');save();
async function run(name,args){
  const log=join(out,name+'.log'),fd=openSync(log,'wx'),start=performance.now();let status;
  try{status=await new Promise((yes,no)=>{const child=spawn(process.execPath,args,{stdio:['ignore',fd,fd]});child.once('error',no);child.once('exit',(code,signal)=>yes({code,signal}));});}
  finally{closeSync(fd);}
  const phase={name,durationMs:performance.now()-start,log,...status};result.phases.push(phase);save();console.log(JSON.stringify(phase));
  assert.equal(status.code,0,`${name} failed; see ${log}`);
}
try{
  if(plan.baseline){
    const baseline=read(plan.baseline);assert(baseline.finishedAt&&baseline.variant==='plain');
    assert(baseline.inputs.files.some(pin=>pin.path===plan.cases&&pin.sha256===hash(readFileSync(plan.cases))),'baseline has another case source');
    result.baseline={path:plan.baseline,sha256:hash(readFileSync(plan.baseline)),reused:true};
  }
  await run('unit',['--test',...plan.tests]);
  const observed=join(out,'observed');
  await run('browser',['--expose-gc',join(directory,'feedback-revision-browser-v32.mjs'),plan.cases,plan.freeze,observed,browserArg,'reads',selection]);
  const plain=plan.baseline??join(out,'plain','results.json');
  if(!plan.baseline)await run('plain',['--expose-gc',join(directory,'feedback-revision-browser-v32.mjs'),plan.cases,plan.freeze,join(out,'plain'),browserArg,'plain',selection]);
  const audit=join(out,'audit.json');
  await run('audit',['--expose-gc',join(directory,'feedback-revision-audit-v19.mjs'),plan.cases,join(observed,'results.json'),plain,audit]);
  result.summary=read(audit).summary;result.browserTimings=read(join(observed,'results.json')).timings;
  result.audit={path:audit,sha256:hash(readFileSync(audit))};result.inputs=[plan.cases,plan.freeze,selection,...plan.tests,new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))}));
  result.finishedAt=new Date().toISOString();result.durationMs=performance.now()-started;save();
  console.log(JSON.stringify({durationMs:result.durationMs,selectedCases:plan.caseIds.length,targets:result.summary.targetHints,noisyControls:result.summary.noisyControls.length,baselineReused:!!plan.baseline}));
}catch(error){result.failure={message:error.message};result.durationMs=performance.now()-started;save();throw error;}
