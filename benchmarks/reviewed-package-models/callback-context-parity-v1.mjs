// Separate evaluator for collector behavior parity on unchanged consumers.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {hash,read} from './catalog.mjs';
const [beforeArg,afterArg,outputArg]=process.argv.slice(2),paths=[resolve(beforeArg),resolve(afterArg)],output=resolve(outputArg);
const [before,after]=paths.map(read);assert(!existsSync(output));assert(before.finishedAt&&after.finishedAt);
assert.deepEqual(before.results.map(row=>row.id),after.results.map(row=>row.id));const changes=[];
for(const row of before.results){const next=after.results.find(item=>item.id===row.id);
  for(const key of ['sourceSha256','originalSourceSha256','publishedTypingErrors','provenance','runtime','packagePins','values'])assert.deepEqual(next[key],row[key],row.id+' '+key);
  assert.equal(next.harnessFailure??null,null);assert.equal(row.harnessFailure??null,null);
  assert.deepEqual(next.observations.filter(note=>note.channelCode),row.observations.filter(note=>note.channelCode));
  assert.deepEqual(next.errors.map(({label,message})=>({label,message})),row.errors.map(({label,message})=>({label,message})));
  assert.deepEqual(next.pageErrors.map(error=>error.message),row.pageErrors.map(error=>error.message));
  if(row.feedback.length!==next.feedback.length)changes.push({id:row.id,oldFeedback:row.feedback.length,newFeedback:next.feedback.length});
  for(const [index,item]of [row,next].entries())assert.equal(hash(readFileSync(join(dirname(paths[index]),item.id,'src/main.tsx'))),item.sourceSha256);
}
writeFileSync(output,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),
  inputs:paths.map(path=>({path,sha256:hash(readFileSync(path))})),consumers:before.results.length,
  checks:['identical original source, labels, published types and packages','identical displayed behavior and callback counts','identical native diagnostic deliveries and caught exceptions'],changes},null,2)+'\n');
console.log(JSON.stringify({consumers:before.results.length,changes}));
