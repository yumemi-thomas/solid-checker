// Reproduce the application's existing opt-in candidate test without source fixes.
// The candidate is unused in the active application; this is an integration trial.
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {closeSync,copyFileSync,existsSync,mkdirSync,openSync,readFileSync,realpathSync,symlinkSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {closurePins,hash,packageRoot} from './catalog.mjs';

const [outArg,variant='candidate',browserPath]=process.argv.slice(2);
assert(outArg&&browserPath&&['candidate','native'].includes(variant));
assert(existsSync(browserPath),'Use the installed browser executable');
const output=resolve(outArg);assert(!existsSync(output));
const baselinePath=resolve('rust/target/real-app-existing-ui-v1/results.json'),baseline=JSON.parse(readFileSync(baselinePath));
const source=baseline.source,clone=join(output,'application');
for(const pin of baseline.inputs.application)assert.equal(hash(readFileSync(join(source,pin.path))),pin.sha256,pin.path);
const packageNames=['@playwright/test','@kobalte/core','vite','vite-plugin-solid','typescript','solid-js','@solidjs/web'];
const packages=packageNames.map(name=>{const root=packageRoot(source,name);return {name,root,version:JSON.parse(readFileSync(join(root,'package.json'))).version,pins:closurePins(root)};});
mkdirSync(clone,{recursive:true});
for(const pin of baseline.inputs.application){const path=join(clone,pin.path);mkdirSync(dirname(path),{recursive:true});copyFileSync(join(source,pin.path),path);}
symlinkSync(realpathSync(join(source,'node_modules')),join(clone,'node_modules'),'dir');
const configPath=join(clone,'research-hydration.config.mjs'),testPath='e2e/kobalte-hydration.spec.ts';
const projectName=variant==='candidate'?'kobalte-alpha':'chromium';
const config=`import original from './playwright.config.ts';
const project=original.projects.find(project=>project.name===${JSON.stringify(projectName)});
export default {...original,fullyParallel:false,workers:1,retries:0,timeout:15000,
  reporter:[['json',{outputFile:${JSON.stringify(join(output,'playwright.json'))}}]],
  outputDir:${JSON.stringify(join(output,'test-artifacts'))},
  projects:[{...project,grep:/hydrates the existing server DOM/,use:{...project.use,headless:true,launchOptions:{executablePath:${JSON.stringify(browserPath)}}}}],
  webServer:[original.webServer[0]]};
`;
writeFileSync(configPath,config);
const env={...process.env,KOBALTE_COMPATIBILITY:variant==='candidate'?'1':'0',SOLID_VIRTUAL_COMPATIBILITY:'0'};
const tsc=join(packageRoot(source,'typescript'),'bin/tsc'),typingArgs=[tsc,'--noEmit','--pretty','false','--project',join(clone,'tsconfig.json')];
const typing=spawnSync(process.execPath,typingArgs,{cwd:clone,env,encoding:'utf8',timeout:60000});
const report={authority:false,certification:false,variant,startedAt:new Date().toISOString(),source,clone,
  scope:'Existing opt-in Kobalte candidate or native control, unchanged source/test/CSP/builds; adapted single-server test selection and installed browser path. No active application defect or package certification claim.',
  inputs:{baseline:{path:baselinePath,sha256:hash(readFileSync(baselinePath))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},application:baseline.inputs.application,packages,
    config:{path:configPath,sha256:hash(config)},browser:{path:browserPath}},
  environment:{KOBALTE_COMPATIBILITY:env.KOBALTE_COMPATIBILITY,SOLID_VIRTUAL_COMPATIBILITY:env.SOLID_VIRTUAL_COMPATIBILITY},
  typing:{args:typingArgs,exitCode:typing.status,signal:typing.signal,output:typing.stdout+typing.stderr},process:null,result:null};
const save=()=>writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');save();
assert.equal(typing.status,0,report.typing.output);
const command=[join(packageRoot(source,'@playwright/test'),'cli.js'),'test',testPath,'--config',configPath];report.command=command;
const log=openSync(join(output,'playwright.log'),'wx');let child;
try{
  child=spawn(process.execPath,command,{cwd:clone,env,detached:true,stdio:['ignore',log,log],timeout:90000});
  report.process=await new Promise(resolveProcess=>{child.once('error',error=>resolveProcess({error:error.message}));child.once('close',(exitCode,signal)=>resolveProcess({exitCode,signal}));});
}finally{
  closeSync(log);
  if(child?.pid)try{process.kill(-child.pid,'SIGTERM');}catch(error){if(error.code!=='ESRCH')throw error;}
}
const resultPath=join(output,'playwright.json');
if(existsSync(resultPath)){const raw=JSON.parse(readFileSync(resultPath));report.result={stats:raw.stats,errors:raw.errors,raw:{path:resultPath,sha256:hash(readFileSync(resultPath))}};}
for(const pin of report.inputs.application){assert.equal(hash(readFileSync(join(source,pin.path))),pin.sha256,pin.path);assert.equal(hash(readFileSync(join(clone,pin.path))),pin.sha256,pin.path);}
for(const pkg of packages)assert.deepEqual(closurePins(pkg.root),pkg.pins,pkg.name);
report.originalAndCopiedInputsUnchanged=true;report.finishedAt=new Date().toISOString();save();
console.log(JSON.stringify({variant,typing:report.typing.exitCode,process:report.process,result:report.result?.stats,originalAndCopiedInputsUnchanged:true}));
