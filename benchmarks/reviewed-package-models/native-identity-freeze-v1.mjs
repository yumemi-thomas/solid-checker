// Seal every prototype module before authoring the new consumer population.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {hash,read} from './catalog.mjs';
const [outArg]=process.argv.slice(2),output=resolve(outArg);assert(!existsSync(output));
const folder=new URL('.',import.meta.url).pathname,baseline=resolve('rust/target/async-read-detector-freeze-v3.json');
const previous=read(baseline);for(const pin of previous.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const files=readdirSync(folder).filter(name=>name.endsWith('.mjs')).map(name=>join(folder,name));files.push(resolve('scripts/tsc-oracle.mjs'));
writeFileSync(output,JSON.stringify({authority:false,certification:false,frozenAt:new Date().toISOString(),baseline:{path:baseline,sha256:hash(readFileSync(baseline))},
  files:files.sort().map(path=>({path,sha256:hash(readFileSync(path))})),scope:'All current prototype modules, including literal browser loads, before fresh cross-file and native-identity cases.'},null,2)+'\n');
console.log(JSON.stringify({files:files.length}));
