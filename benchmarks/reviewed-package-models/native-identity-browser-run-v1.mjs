// Freeze original consumers and all prototype modules across each browser run.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,readdirSync,writeFileSync,openSync,closeSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {spawnSync} from 'node:child_process';
import {hash,read,closurePins,packageRoot} from './catalog.mjs';
const [casesArg,outArg,browser,variant='identity']=process.argv.slice(2),out=resolve(outArg),casesPath=resolve(casesArg);
assert(browser&&!existsSync(out));assert(['plain','identity'].includes(variant));mkdirSync(out,{recursive:true});
const folder=new URL('.',import.meta.url).pathname,cases=(await import(pathToFileURL(casesPath))).default;
const files=readdirSync(folder).filter(name=>name.endsWith('.mjs')).map(name=>join(folder,name));files.push(resolve('scripts/tsc-oracle.mjs'));
const tooling=resolve('rust/target/app-import-metric/apps/helge-dev'),roots=new Set();
for(const item of cases)for(const name of item.packages??[item.package])roots.add(packageRoot(resolve('rust/target/app-import-metric/apps',item.app),name));
for(const name of ['vite','@solidjs/vite-plugin','playwright','@jridgewell/trace-mapping'])roots.add(packageRoot(tooling,name));
const packages=()=>[...roots].map(root=>({root,pins:closurePins(root)})),snapshot=()=>files.sort().map(path=>({path,sha256:hash(readFileSync(path))}));
const before={authority:false,certification:false,variant,files:snapshot(),packages:packages()};
writeFileSync(join(out,'inputs-before.json'),JSON.stringify(before,null,2)+'\n');
const log=openSync(join(out,'browser.log'),'wx');let result;
try{result=spawnSync(process.execPath,[join(folder,'browser-experiment-v5.mjs'),join(out,'browser'),browser],{
  env:{...process.env,REVIEWED_MODEL_INPUT_SELECTION:'',REVIEWED_MODEL_BROWSER_CASES:casesPath,REVIEWED_MODEL_FEEDBACK_BRIDGE:join(folder,'family-matrix-feedback-v2.mjs'),
    REVIEWED_MODEL_SOURCE_PLUGIN:variant==='plain'?'':join(folder,'async-read-transform-v5.mjs'),REVIEWED_MODEL_ATTRIBUTION:'1',REVIEWED_MODEL_ATTRIBUTION_PREBUNDLE:'',
    REVIEWED_MODEL_GUARD_TRACE:'',REVIEWED_MODEL_ORIGIN_TRACE:'',REVIEWED_MODEL_DISABLE_CORE_PREBUNDLE:'1'},
  stdio:['ignore',log,log],timeout:360000});}finally{closeSync(log);}
assert.equal(result.error,undefined);assert.equal(result.status,0);assert.deepEqual(snapshot(),before.files);assert.deepEqual(packages(),before.packages);
writeFileSync(join(out,'inputs-after.json'),JSON.stringify({...before,finishedAt:new Date().toISOString()},null,2)+'\n');
console.log(JSON.stringify({variant,consumers:read(join(out,'browser/results.json')).results.length,files:before.files.length}));
