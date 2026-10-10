// Supplement the import-only seal with earlier seals for literal runtime loads.
// Keep the original study's conservative false flag; never rewrite old evidence.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {hash,read} from './catalog.mjs';
const [populationArg,browserArg,outputArg]=process.argv.slice(2),populationPath=resolve(populationArg),browserPath=resolve(browserArg),output=resolve(outputArg);
const population=read(populationPath),browser=read(browserPath),detector=read(population.detector.path);assert(!existsSync(output));
const profile=dirname(dirname(browserPath)),before=read(join(profile,'inputs-before.json')),after=read(join(profile,'inputs-after.json'));
assert.deepEqual(before.files,after.files);assert(new Date(population.frozenAt)<new Date(browser.startedAt));
const oldProfile=resolve('rust/target/snapshot-helper-caller-browser-v1'),oldBrowser=read(join(oldProfile,'browser/results.json')),
  oldBefore=read(join(oldProfile,'inputs-before.json')),oldAfter=read(join(oldProfile,'inputs-after.json'));
assert.deepEqual(oldBefore.files,oldAfter.files);assert(new Date(oldBrowser.finishedAt)<new Date(population.frozenAt));
const baseline=read(detector.baseline.path);assert(new Date(baseline.frozenAt)<new Date(population.frozenAt));
assert.equal(hash(readFileSync(detector.baseline.path)),detector.baseline.sha256);
const seals=[{path:population.detector.path,frozenAt:detector.frozenAt,files:detector.files},
  {path:detector.baseline.path,frozenAt:baseline.frozenAt,files:baseline.files},
  {path:join(oldProfile,'inputs-before.json'),frozenAt:oldBrowser.finishedAt,files:oldBefore.files}];
for(const seal of seals)for(const pin of seal.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const admitted=[],missing=[];
for(const pin of before.files.filter(pin=>pin.path!==population.caseModule)){
  const seal=seals.find(seal=>new Date(seal.frozenAt)<new Date(population.frozenAt)&&seal.files.some(old=>old.path===pin.path&&old.sha256===pin.sha256));
  if(seal)admitted.push({...pin,earlierSeal:seal.path,frozenAt:seal.frozenAt});else missing.push(pin);
}
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),
  inputs:[populationPath,browserPath,...seals.map(seal=>seal.path),join(oldProfile,'browser/results.json')]
    .map(path=>({path,sha256:hash(readFileSync(path))})),
  combinedEarlierSealsCoverProfile:missing.length===0,admitted,missing,
  limit:'Import-only freeze missed two literal runtime modules; coverage here uses authenticated historical seals strictly before this population. Adapted files remain missing.'};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({covered:admitted.length,missing:missing.map(pin=>pin.path),complete:report.combinedEarlierSealsCoverProfile}));
