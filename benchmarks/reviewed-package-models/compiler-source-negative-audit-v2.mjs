import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {hash} from './catalog.mjs';
const [inputArg,bindingsArg,outArg]=process.argv.slice(2),input=resolve(inputArg),bindingsPath=resolve(bindingsArg),out=resolve(outArg);
assert(!existsSync(out));mkdirSync(out);const original=JSON.parse(readFileSync(bindingsPath));
const changes={
  'foreign-overload':value=>{const binding=value.bindings.find(row=>row.resolution.kind==='checked-overload-signature');binding.declaration.span.end++;},
  'unchecked-selected-overload':value=>{const binding=value.bindings.find(row=>row.resolution.kind==='declared-overload-family');binding.declarationSet.selectedOverload=0;},
  'missing-source-decision':value=>{value.decisions.pop();},
  'invented-package-typing':value=>{value.summary.reusedCleanTypingRuns++;},
  'runtime-provider-claim':value=>{value.bindings[0].runtimeProvider='proven';},
  'callback-timing-claim':value=>{value.bindings[0].callbackInvocation='tracked';},
  'wrong-package-owner':value=>{value.bindings[0].declaration.owner.name='wrong-package';},
  'drifting-output-binding':value=>{value.bindings[0].file=value.open.find(row=>row.reason==='installed-output-differs').file;}
};
const rows=[];
for(const [name,change] of Object.entries(changes)){
  const value=structuredClone(original);change(value);const path=join(out,name+'.json'),audit=join(out,name+'.audit.json');writeFileSync(path,JSON.stringify(value,null,2)+'\n');
  const child=spawnSync(process.execPath,[fileURLToPath(new URL('./compiler-source-audit-v2.mjs',import.meta.url)),input,path,audit],{encoding:'utf8',timeout:60000,maxBuffer:1024*1024});
  assert(child.status!==0&&child.status!==null,name);assert(!existsSync(audit),name);
  rows.push({name,exitCode:child.status,signal:child.signal,stderr:child.stderr,refused:true});
}
const result={authority:false,certification:false,refused:rows.length,rows,inputs:[input,bindingsPath,fileURLToPath(import.meta.url)].map(path=>({path,sha256:hash(readFileSync(path))}))};
writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({refused:rows.length}));
