// Full copied-app parity; the single-consumer auditor assumes src/main.tsx.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {dirname,join,relative,resolve} from 'node:path';
import {hash,read} from './catalog.mjs';

const [beforeArg,afterArg,outArg]=process.argv.slice(2),beforePath=resolve(beforeArg),afterPath=resolve(afterArg),out=resolve(outArg),before=read(beforePath),after=read(afterPath);
assert(!existsSync(out));assert(before.finishedAt&&after.finishedAt);assert.equal(before.results.length,after.results.length);
const rows=[];
function sourceTree(root){const files=[];function walk(path){for(const entry of readdirSync(path,{withFileTypes:true})){const file=join(path,entry.name);if(entry.isDirectory())walk(file);else{assert(entry.isFile());files.push({path:relative(root,file),sha256:hash(readFileSync(file))});}}}walk(join(root,'src'));return files.sort((a,b)=>a.path.localeCompare(b.path));}
function authenticate(path){const profile=dirname(dirname(path)),start=read(join(profile,'inputs-before.json')),end=read(join(profile,'inputs-after.json'));assert.deepEqual(start.files,end.files);assert.deepEqual(start.packages,end.packages);
 for(const pin of start.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);return start;}
const inputsBefore=authenticate(beforePath),inputsAfter=authenticate(afterPath);assert.deepEqual(inputsBefore.packages,inputsAfter.packages);
for(const baseline of before.results){const revised=after.results.find(row=>row.id===baseline.id);assert(revised);
 const rootBefore=join(dirname(beforePath),baseline.id),rootAfter=join(dirname(afterPath),baseline.id);
 assert.deepEqual(sourceTree(rootBefore),sourceTree(rootAfter));
 const normalize=value=>JSON.parse(JSON.stringify(value).replaceAll(rootBefore,'<app>').replaceAll(rootAfter,'<app>')
  .replace(/http:\/\/127\.0\.0\.1:\d+/g,'http://browser').replace(/\?v=[a-f0-9]+/g,''));
 for(const key of ['sourceSha256','originalSourceSha256','provenance','packagePins','publishedTypingErrors','values','disposals','observations','errors','pageErrors','windowErrors','feedback','harnessFailure'])assert.deepEqual(normalize(baseline[key]??null),normalize(revised[key]??null),key);
 assert.deepEqual(baseline.steps.map(step=>step.label),revised.steps.map(step=>step.label));
 for(const file of revised.sourceInstrumentation.transformed)assert.equal(hash(readFileSync(file.path)),file.sourceSha256,file.path);
 assert.equal(revised.sourceInstrumentation.refused.length,0);
 rows.push({id:baseline.id,sourceFiles:sourceTree(rootBefore).length,executedSteps:revised.steps.map(step=>step.label),
  nativeMessages:revised.feedback.map(note=>({code:note.code,category:note.category,hasConsumerLocation:!!note.originalLocation})),readEvents:revised.readTrace.length,
  transformedFiles:revised.sourceInstrumentation.transformed.length,syntaxSkipped:revised.sourceInstrumentation.transformed.filter(file=>file.session?.skipped).length,
  programGenerations:[...new Set(revised.sourceInstrumentation.transformed.map(file=>file.session?.generation).filter(Boolean))],
  mountBeforeMs:baseline.steps.find(step=>step.label==='mount').durationMs,mountAfterMs:revised.steps.find(step=>step.label==='mount').durationMs});
}
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:[beforePath,afterPath].map(path=>({path,sha256:hash(readFileSync(path))})),rows,changes:[],
 limitations:['single browser execution per variant; no statistically established speedup','full app flows observed; no guarantee every route or callback ran','native advisory messages retained; silence is not correctness']},null,2)+'\n');
console.log(JSON.stringify(rows));
