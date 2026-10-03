// Analyze original executed files only; no synthetic package analog is run.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { performance } from 'node:perf_hooks';
import { hash, read, packageRoot, closurePins } from './catalog.mjs';
import { familyFeedback } from './family-feedback-system.mjs';
import { scoreHoldout } from './family-holdout-score.mjs';
import { ts } from './lower.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [browserArg, outputArg, filterArg] = process.argv.slice(2), browserPath=resolve(browserArg), output=resolve(outputArg), browser=read(browserPath);
assert(browser.finishedAt);assert(!existsSync(output));mkdirSync(output,{recursive:true});
const checker=resolve('rust/target/debug/solid-checker-rust'),producer=resolve('bin/solid-typefacts');
const pins=[browserPath,checker,producer,resolve('bin/solid-typefacts.buildinfo'),new URL(import.meta.url).pathname]
  .map(path=>({path,sha256:hash(readFileSync(path))}));
const originalFreeze=read('rust/target/family-holdout-detector-freeze.json');
for(const path of [checker,producer,resolve('bin/solid-typefacts.buildinfo')])assert(originalFreeze.files.some(pin=>pin.path===path&&pin.sha256===hash(readFileSync(path))));
const results=[],report={authority:false,certification:false,startedAt:new Date().toISOString(),finishedAt:null,inputs:pins,results};
const save=()=>writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');
for(const row of browser.results.filter(row=>filterArg?.split(',').includes(row.id))){
  const root=join(dirname(browserPath),row.id),path=join(root,'src/main.tsx');assert.equal(hash(readFileSync(path)),row.sourceSha256);
  for(const pkg of row.packagePins)assert.deepEqual(closurePins(packageRoot(root,pkg.package)),pkg.pins);
  const options=oracleCompilerOptions('v2',true,{customConditions:['browser','development']});
  const program=ts.createProgram([path],ts.convertCompilerOptionsFromJson(options,root).options);
  const diagnostics=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.deepEqual(diagnostics.map(d=>d.code).sort(),row.publishedTypingErrors.map(d=>d.code).sort());
  const dir=join(output,row.id);mkdirSync(dir);const config=join(dir,'tsconfig.json');
  writeFileSync(config,JSON.stringify({compilerOptions:options,files:[path]},null,2)+'\n');
  let baseline=null,durationMs=0;
  if(!diagnostics.length){const started=performance.now();
    const run=spawnSync(checker,['--format','json','--runtime-target','browser','--project',config],
      {env:{...process.env,SOLID_TYPEFACTS_BIN:producer,SOLID_CHECKER_DAEMON:'0'},encoding:'utf8',timeout:30000,maxBuffer:32*1024*1024});
    durationMs=performance.now()-started;assert(!run.error,run.error?.message);assert([0,1].includes(run.status),run.stderr);
    baseline=JSON.parse(run.stdout);writeFileSync(join(dir,'native.json'),run.stdout);
  }
  const combined=familyFeedback({publishedTypingErrors:row.publishedTypingErrors,feedback:row.feedback,errors:row.errors,pageErrors:row.pageErrors,windowErrors:row.windowErrors},{baseline});
  results.push({id:row.id,package:row.packagePins[0].package,provenance:row.provenance,...combined,
    publishedTypingErrors:row.publishedTypingErrors,harnessFailure:row.harnessFailure??null,
    declaredBehavior:row.values?.behavior&&{...row.values.behavior,passed:row.values.behavior.actual===row.values.behavior.desired},
    nativeFindings:baseline?.findings??[],durationMs,sourceSha256:row.sourceSha256});save();
  console.log(JSON.stringify({id:row.id,durationMs,violations:baseline?.findings.filter(f=>f.kind==='violation').map(f=>f.rule),gaps:combined.gaps.map(f=>f.rule??f.id)}));
}
assert(results.length);for(const pin of pins)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
report.summary=scoreHoldout(results);report.finishedAt=new Date().toISOString();save();
