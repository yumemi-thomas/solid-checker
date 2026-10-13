// Independently bind existing test outcomes to exact browser and source evidence.
// Artifact ownership records a file's package; it never assigns causal blame.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {closurePins,hash,packageRoot} from './catalog.mjs';

const args=process.argv.slice(2),output=resolve(args.pop());assert.equal(args.length,4);assert(!existsSync(output));
const inputs=args.map(path=>({path:resolve(path),report:JSON.parse(readFileSync(path))}));
const tooling=resolve('rust/target/app-import-metric/apps/helge-dev');
const {TraceMap,originalPositionFor}=await import(pathToFileURL(join(packageRoot(tooling,'@jridgewell/trace-mapping'),'dist/trace-mapping.mjs')));
const summaries=[];
function pin(pin){assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);}
function specs(suite){return [...(suite.specs??[]),...(suite.suites??[]).flatMap(specs)];}
function sourceOwner(path){
  for(let folder=dirname(path);;folder=dirname(folder)){
    const manifest=join(folder,'package.json');
    if(existsSync(manifest)){const bytes=readFileSync(manifest),data=JSON.parse(bytes);return {name:data.name,version:data.version,root:realpathSync(folder),manifest,manifestSha256:hash(bytes)};}
    assert.notEqual(dirname(folder),folder,'Mapped source lacks a package manifest');
  }
}
for(let index=0;index<2;index++){
  const test=inputs[index],observed=inputs[index+2],run=test.report,observation=observed.report,variant=index===0?'candidate':'native';
  assert.equal(run.variant,variant);assert.equal(observation.variant,variant);
  for(const report of [run,observation]){assert.equal(report.authority,false);assert.equal(report.certification,false);assert(report.finishedAt);assert(!report.failure);pin(report.inputs.runner);}
  pin(observation.inputs.run);assert.equal(observation.inputs.run.path,test.path);
  pin(run.inputs.baseline);pin(run.inputs.config);assert(run.originalAndCopiedInputsUnchanged);
  assert.equal(run.typing.exitCode,0);assert.equal(run.typing.signal,null);assert.equal(run.typing.output,'');
  assert(run.typing.args.includes('--noEmit'));assert(run.typing.args.includes(join(run.clone,'tsconfig.json')));
  assert.equal(run.typing.args[0],join(run.inputs.packages.find(pkg=>pkg.name==='typescript').root,'bin/tsc'));
  for(const input of run.inputs.application){assert.equal(hash(readFileSync(join(run.source,input.path))),input.sha256);assert.equal(hash(readFileSync(join(run.clone,input.path))),input.sha256);}
  for(const pkg of [...run.inputs.packages,...observation.inputs.tools])assert.deepEqual(closurePins(pkg.root),pkg.pins,pkg.name);
  assert.equal(run.environment.KOBALTE_COMPATIBILITY,variant==='candidate'?'1':'0');assert.equal(run.environment.SOLID_VIRTUAL_COMPATIBILITY,'0');
  assert.equal(run.inputs.packages.find(pkg=>pkg.name==='@kobalte/core').version,'2.0.0-alpha.2');
  assert.equal(run.inputs.packages.find(pkg=>pkg.name==='solid-js').version,'2.0.0-rc.9');
  const rawPath=run.result.raw.path;pin(run.result.raw);const raw=JSON.parse(readFileSync(rawPath)),selected=raw.suites.flatMap(specs);
  assert.deepEqual(raw.stats,run.result.stats);assert.deepEqual(raw.errors,[]);assert.equal(selected.length,1);
  const selectedSpec=selected[0];assert.equal(selectedSpec.file,'kobalte-hydration.spec.ts');assert.equal(selectedSpec.line,22);assert.equal(selectedSpec.tests.length,1);
  const originalLines=readFileSync(join(run.source,'e2e',selectedSpec.file),'utf8').split('\n');assert(originalLines[selectedSpec.line-1].includes(selectedSpec.title));
  const result=selectedSpec.tests[0];assert.equal(result.projectName,variant==='candidate'?'kobalte-alpha':'chromium');assert.equal(result.results.length,1);assert.equal(result.results[0].retry,0);
  assert.deepEqual({expected:raw.stats.expected,skipped:raw.stats.skipped,unexpected:raw.stats.unexpected,flaky:raw.stats.flaky},variant==='candidate'?{expected:0,skipped:0,unexpected:1,flaky:0}:{expected:1,skipped:0,unexpected:0,flaky:0});
  assert.deepEqual(run.process,{exitCode:variant==='candidate'?1:0,signal:null});
  assert.equal(result.results[0].status,variant==='candidate'?'failed':'passed');
  if(variant==='candidate'){
    assert.deepEqual(result.results[0].errors.map(error=>error.location.line),[15,19]);
    assert(originalLines[14].includes('data-hydrated'));assert(originalLines[18].includes('toEqual([])'));
  }
  assert.deepEqual(observation.blockedRequests,[]);assert.equal(observation.response.status,200);assert(!observation.response.csp.includes('unsafe-inline'));
  assert(observation.response.csp.includes("default-src 'self'; script-src 'nonce-"));
  assert.equal(observation.server.exitCode,0);assert.equal(observation.server.signal,null);assert.equal(observation.server.originalInput,'e2e/fixtures/server.ts');
  const folder=dirname(observed.path),served=readFileSync(join(folder,'served-client.js')),mappedCode=readFileSync(join(folder,'mapped-client.js'));
  assert.deepEqual(served,mappedCode);assert.equal(observation.clientByteIdentity.matched,true);
  assert.equal(hash(served),observation.clientByteIdentity.servedSha256);assert.equal(hash(mappedCode),observation.clientByteIdentity.mappedSha256);
  assert.equal(hash(mappedCode),observation.map.codeSha256);assert.equal(hash(readFileSync(observation.map.file)),observation.map.sha256);
  const map=JSON.parse(readFileSync(observation.map.file)),trace=new TraceMap(map);assert.equal(map.sources.length,observation.map.sources.length);
  for(const [sourceIndex,source] of observation.map.sources.entries()){
    assert.equal(source.status,'exact');assert.equal(source.name,map.sources[sourceIndex]);
    assert.equal(source.path,resolve(run.clone,'dist',map.sourceRoot??'',source.name));
    const bytes=readFileSync(source.path,'utf8');assert.equal(bytes,map.sourcesContent[sourceIndex]);assert.equal(hash(bytes),source.sourceSha256);
    assert.equal(source.owner.staticDispatch,'open');const owner=sourceOwner(source.path);
    for(const [key,value] of Object.entries(owner))assert.equal(source.owner[key],value);
  }
  const exactFrames=[];
  for(const diagnostic of observation.diagnostics){
    if(variant==='candidate')assert(result.results[0].errors[1].message.includes(diagnostic.text??diagnostic.message));
    const reported=diagnostic.frames??[],captured=[];
    if(diagnostic.location?.url)captured.push({url:diagnostic.location.url,line:diagnostic.location.lineNumber+1,column:diagnostic.location.columnNumber+1});
    for(const line of (diagnostic.stack??'').split('\n')){const match=/(https?:\/\/[^\s)]+):(\d+):(\d+)\)?$/.exec(line);if(match)captured.push({url:match[1],line:Number(match[2]),column:Number(match[3])});}
    assert.equal(captured.length,reported.length);
    for(const [frameIndex,frame] of reported.entries()){
      const coordinates=captured[frameIndex];for(const [key,value] of Object.entries(coordinates))assert.equal(frame[key],value);
      if(new URL(frame.url).pathname!=='/client.js'){assert.equal(frame.status,'outside-client-bundle');continue;}
      assert.equal(frame.status,'exact');const original=originalPositionFor(trace,{line:frame.line,column:frame.column-1}),source=observation.map.sources.find(source=>source.name===original.source);
      assert(source);assert.equal(frame.original.path,source.path);assert.equal(frame.original.line,original.line);assert.equal(frame.original.column,original.column+1);assert.equal(frame.original.name,original.name);
      assert.equal(frame.original.sourceSha256,source.sourceSha256);assert.deepEqual(frame.original.owner,source.owner);exactFrames.push(frame.original);
    }
  }
  assert.equal(observation.snapshot.hydrated,variant==='candidate'?null:'true');assert.equal(observation.snapshot.rootReused,true);assert.equal(observation.snapshot.triggerReused,true);
  const diagnosticGroups={hydration:observation.diagnostics.filter(d=>d.kind==='pageerror'&&d.message.startsWith('Hydration Mismatch.')).length,halted:observation.diagnostics.filter(d=>d.text==='[REACTIVITY_HALTED]').length,csp:observation.diagnostics.filter(d=>d.text?.startsWith('Applying inline style violates')).length};
  assert.deepEqual(diagnosticGroups,variant==='candidate'?{hydration:2,halted:2,csp:4}:{hydration:0,halted:0,csp:0});
  assert.equal(observation.diagnostics.length,variant==='candidate'?8:0);assert.equal(exactFrames.length,variant==='candidate'?22:0);
  const packageLocations=[...new Map(exactFrames.map(frame=>[`${frame.path}:${frame.line}:${frame.column}`,frame])).values()];
  const feedback=variant==='candidate'?[{kind:'informational',claim:'The selected existing integration fails its hydration prerequisite and clean-console assertion while its configured TypeScript project passes.',
    test:{path:join(run.source,'e2e',selectedSpec.file),line:22,failedAssertionLines:[15,19],testBodyReached:false},
    sourceLocations:packageLocations,causalPackage:'open',staticDispatch:'open',repair:'open',certification:false}]:[];
  summaries.push({variant,test:{path:test.path,sha256:hash(readFileSync(test.path))},observation:{path:observed.path,sha256:hash(readFileSync(observed.path))},
    typingExitCode:0,testOutcome:result.results[0].status,skipped:0,sourceFiles:map.sources.length,mappedFrames:exactFrames.length,diagnosticGroups,
    sourceLocationPackages:[...new Set(exactFrames.map(frame=>frame.owner.name))],bundlePackages:[...new Set(observation.map.sources.map(source=>source.owner.name))],feedback});
}
assert.deepEqual(inputs[0].report.inputs.application,inputs[1].report.inputs.application);
assert.deepEqual(inputs[0].report.inputs.packages,inputs[1].report.inputs.packages);
const audit={authority:false,certification:false,auditedAt:new Date().toISOString(),summaries,
  limits:['Existing unused candidate integration; no new defect in the active application is claimed.','One existing failing test and one native passing control; no general warning precision or coverage rate.','All observed client stack frames map to Solid runtime artifacts; Kobalte is present in the candidate bundle but causal responsibility and export dispatch remain open.','Browser CSP diagnostics point to document locations and remain outside the client map.','No source repair or per-package behavioral contract.']};
writeFileSync(output,JSON.stringify(audit,null,2)+'\n');console.log(JSON.stringify(summaries.map(({variant,testOutcome,mappedFrames,diagnosticGroups,feedback})=>({variant,testOutcome,mappedFrames,diagnosticGroups,feedbackNotes:feedback.length}))));
