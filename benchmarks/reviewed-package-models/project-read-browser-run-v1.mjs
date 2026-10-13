// Frozen original-app profile; all browser work happens in a fresh copy.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {spawnSync} from 'node:child_process';
import {hash,read,closurePins,packageRoot} from './catalog.mjs';

const [outArg,browser,variant='v3',casesArg='']=process.argv.slice(2),out=resolve(outArg);
assert(browser&&!existsSync(out));assert(['plain','v2','v3'].includes(variant));mkdirSync(out,{recursive:true});
const folder=new URL('.',import.meta.url).pathname,app=resolve('rust/target/app-import-metric/apps/helge-dev'),files=new Set();
for(const name of readdirSync(folder).filter(name=>name.endsWith('.mjs')))files.add(join(folder,name));
files.add(resolve('scripts/tsc-oracle.mjs'));
function walk(path){for(const entry of readdirSync(path,{withFileTypes:true})){const child=join(path,entry.name);if(entry.isDirectory())walk(child);else if(entry.isFile())files.add(child);}}
walk(join(app,'src'));walk(join(app,'static'));
for(const name of ['index.html','package.json','tsconfig.json'])files.add(join(app,name));
if(casesArg)files.add(resolve(casesArg));
const pkgNames=['solid-js','@solidjs/web','@solidjs/router','@solidjs/meta','solid-icons','vite','@solidjs/vite-plugin','playwright'];
const packages=()=>pkgNames.map(name=>({name,pins:closurePins(packageRoot(app,name))}));
const snapshot=()=>[...files].sort().map(path=>({path,sha256:hash(readFileSync(path))}));
const before={authority:false,certification:false,variant,files:snapshot(),packages:packages()};
writeFileSync(join(out,'inputs-before.json'),JSON.stringify(before,null,2)+'\n');
const log=openSync(join(out,'browser.log'),'wx');let result;
try{result=spawnSync(process.execPath,[join(folder,'browser-experiment-v4.mjs'),join(out,'browser'),browser,...(casesArg?[]:['helge-app'])],{
 env:{...process.env,REVIEWED_MODEL_BROWSER_CASES:casesArg?resolve(casesArg):'',REVIEWED_MODEL_FEEDBACK_BRIDGE:join(folder,'family-matrix-feedback-v2.mjs'),
  REVIEWED_MODEL_SOURCE_PLUGIN:variant==='plain'?'':join(folder,`async-read-transform-${variant}.mjs`),REVIEWED_MODEL_ATTRIBUTION:'1',REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE:'',
  REVIEWED_MODEL_GUARD_TRACE:'',REVIEWED_MODEL_ORIGIN_TRACE:'',REVIEWED_MODEL_DISABLE_CORE_PREBUNDLE:''},
 stdio:['ignore',log,log],timeout:300000});}finally{closeSync(log);}
assert.equal(result.error,undefined);assert.equal(result.status,0);assert.deepEqual(snapshot(),before.files);assert.deepEqual(packages(),before.packages);
writeFileSync(join(out,'inputs-after.json'),JSON.stringify({...before,finishedAt:new Date().toISOString()},null,2)+'\n');
console.log(JSON.stringify({variant,results:read(join(out,'browser/results.json')).results.length,sourceAndToolFiles:before.files.length}));
