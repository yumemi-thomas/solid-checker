import assert from 'node:assert/strict';
import {existsSync,mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {defaults,directory,historical} from './profile.mjs';
import {execute} from './process.mjs';
import {hash,read} from '../catalog.mjs';
const [outArg,browserArg,configArg,engine='chromium']=process.argv.slice(2),config=configArg?read(resolve(configArg)):defaults;
assert(config.hypothesis?.trim()&&config.success?.trim(),'Each slice needs a hypothesis and success criterion');
assert(config.caseIds?.length&&config.tests?.length);
const plan={...config,developmentOnly:true,engine};
if(outArg==='--plan'){console.log(JSON.stringify(plan,null,2));process.exit(0);}
const out=resolve(outArg);assert(browserArg&&!existsSync(out));mkdirSync(out,{recursive:true});
const selection=join(out,'selection.json');writeFileSync(selection,JSON.stringify({caseIds:config.caseIds})+'\n');
const started=performance.now(),result={authority:false,certification:false,developmentOnly:true,startedAt:new Date().toISOString(),plan,phases:[]};
const save=()=>writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2)+'\n');save();
async function run(name,args,memory=false){const phase={name,...await execute(args,join(out,name+'.log'),{memory})};result.phases.push(phase);save();assert.equal(phase.code,0,phase.log);console.log(JSON.stringify({phase:name,wallMs:phase.wallMs}));return phase;}
try{
  if(!config.freeze){config.freeze=join(out,'working-freeze.json');await run('freeze',[join(directory,'freeze.mjs'),config.freeze]);}
  await run('unit',['--test',...config.tests]);
  const browser=join(directory,'browser.mjs'),observed=join(out,'observed');
  await run('browser',['--expose-gc',browser,config.cases,config.freeze,observed,browserArg,'reads',selection,engine],true);
  const plain=config.baseline??join(out,'plain','results.json');
  if(!config.baseline)await run('plain',['--expose-gc',browser,config.cases,config.freeze,join(out,'plain'),browserArg,'plain',selection,engine]);
  const audit=join(out,'audit.json');
  await run('audit',['--expose-gc',join(historical,'feedback-revision-audit-v19.mjs'),config.cases,join(observed,'results.json'),plain,audit]);
  result.summary=read(audit).summary;result.browserTimings=read(join(observed,'results.json')).timings;
  result.audit={path:audit,sha256:hash(readFileSync(audit))};result.finishedAt=new Date().toISOString();result.wallMs=performance.now()-started;save();
  console.log(JSON.stringify({wallMs:result.wallMs,selectedCases:config.caseIds.length,summary:result.summary}));
}catch(error){result.failure={message:error.message};result.wallMs=performance.now()-started;save();throw error;}
