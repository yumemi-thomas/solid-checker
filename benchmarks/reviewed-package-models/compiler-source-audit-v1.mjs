// Independent source/output/operation reconciliation for the compiler experiment.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {hash,packageDigest} from './catalog.mjs';

const [inputArg,bindingsArg,outArg]=process.argv.slice(2),input=resolve(inputArg),bindingsPath=resolve(bindingsArg),output=resolve(outArg);
assert(!existsSync(output));const report=JSON.parse(readFileSync(input)),bindings=JSON.parse(readFileSync(bindingsPath));
function pin(value){assert.equal(hash(readFileSync(value.path)),value.sha256,value.path);}
function ownership(path){for(let dir=dirname(path);;dir=dirname(dir)){const manifest=join(dir,'package.json');if(existsSync(manifest)){const data=JSON.parse(readFileSync(manifest));return {name:data.name,version:data.version,root:realpathSync(dir),manifestSha256:hash(readFileSync(manifest))};}if(dirname(dir)===dir)return null;}}
pin(report.inputs.runner);pin(report.inputs.seal);pin(report.inputs.probe);pin(report.inputs.compilerIdentity);pin(report.inputs.loadedBinary);
for(const value of report.inputs.probeSource)pin(value);
for(const value of report.inputs.observations)pin(value);
for(const value of report.inputs.installed)assert.equal(packageDigest(value.root),value.digest);
pin(bindings.inputs.runner);pin(bindings.inputs.report);assert.equal(bindings.inputs.report.path,input);
const identity=JSON.parse(readFileSync(report.inputs.compilerIdentity.path)),folder=dirname(input),raw=JSON.parse(readFileSync(join(folder,'probe.json'))),requests=JSON.parse(readFileSync(join(folder,'requests.json')));
assert.equal(raw.length,requests.length);assert.equal(raw.length,report.rows.length+2);
const artifacts=new Map();let sourceOperations=0,generatedOperations=0,matching=0;
const contexts={dom:{},ssr:{}};
for(const [index,row] of report.rows.entries()){
  const path=join(folder,row.file),artifact=JSON.parse(readFileSync(path)),facts=artifact.facts,model=facts.executionMap.semanticModel,source=readFileSync(row.path);
  assert.deepEqual(facts,raw[index]);assert.deepEqual(artifact.request,requests[index]);assert.equal(facts.status,'facts');
  assert.equal(facts.path,artifact.request.path);assert.deepEqual(facts.options,artifact.request.options);assert.equal(facts.options.generate,row.generate);
  assert.equal(facts.authority,false);assert.equal(facts.certification,false);assert.equal(facts.executionMap.compilerFactsProtocol,2);
  assert.equal(hash(source),row.sourceSha256);assert.equal(facts.executionMap.sourceHash,row.sourceSha256);
  assert.equal(model.producer.implementationRevision,identity.implementationRevision);assert.equal(model.producer.upstreamRevision,identity.upstreamRevision);
  assert.equal(model.producer.traceVersion,3);assert.equal(model.producer.identityComplete,true);
  assert.equal('sha256:'+model.producer.outputSha256,hash(facts.output));assert.equal(facts.sourceMap,null);
  assert.equal(model.sourceOperationsComplete,true);assert.equal(model.generatedOperationsComplete,false);assert.equal(facts.traceOnOffOutputIdentity,true);
  assert.equal(facts.output===artifact.installed.code,row.outputByteIdentity);if(row.outputByteIdentity)matching++;
  assert.equal(row.sourceOperations,model.operations.length);assert.equal(row.generatedOperations,model.generatedOperations.length);
  const generated=new Map(model.generatedOperations.map(operation=>[operation.id,operation]));assert.equal(generated.size,model.generatedOperations.length);
  assert.equal(new Set(model.operations.map(operation=>operation.id)).size,model.operations.length);
  for(const operation of [...model.operations,...model.generatedOperations]){
    const span=operation.span??operation.sourceSpan;assert(Number.isInteger(span.start)&&Number.isInteger(span.end)&&span.start>=0&&span.end>=span.start&&span.end<=source.length);
    if(operation.execution){for(const id of operation.execution.generatedOperations)assert(generated.has(id));
      const kind=operation.execution.disposition;contexts[row.generate][kind]=(contexts[row.generate][kind]??0)+1;}
  }
  sourceOperations+=model.operations.length;generatedOperations+=model.generatedOperations.length;artifacts.set(row.file,{path,artifact,row,source,model});
}
assert.equal(sourceOperations,report.summary.sourceOperations);assert.equal(generatedOperations,report.summary.generatedOperations);assert.equal(matching,report.summary.outputMatches);
assert(raw.slice(report.rows.length).every(row=>row.status==='refused'));assert.deepEqual(raw.slice(report.rows.length),report.refusals);
const seen=new Set();let exactContexts=0,openContexts=0,externalDeclarations=0;
for(const binding of bindings.bindings){
  const key=`${binding.file}:${binding.generated.id}`;assert(!seen.has(key));seen.add(key);
  const {artifact,row,source,model,path}=artifacts.get(binding.file);assert(row.outputByteIdentity);
  pin(binding.artifact);assert.equal(binding.artifact.path,path);assert.equal(binding.source.path,row.path);assert.equal(binding.source.sha256,row.sourceSha256);
  assert.deepEqual(binding.generated,model.generatedOperations.find(operation=>operation.id===binding.generated.id));assert.equal(binding.generated.kind,'component-invocation');
  assert.deepEqual(binding.source.span,binding.generated.sourceSpan);
  assert.equal(source.subarray(binding.tag.span.start,binding.tag.span.end).toString(),binding.tag.text);
  assert(binding.tag.span.start>=binding.source.span.start&&binding.tag.span.end<=binding.source.span.end);
  assert.equal(binding.finding,false);assert.equal(binding.runtimeProvider,'open');assert.equal(binding.callbackInvocation,'open');assert.equal(binding.causalResponsibility,'open');
  const declaration=readFileSync(binding.declaration.path);assert.equal(hash(declaration),binding.declaration.sourceSha256);
  assert(binding.declaration.span.start>=0&&binding.declaration.span.end<=declaration.length);
  assert.deepEqual(binding.declaration.owner,JSON.parse(JSON.stringify(ownership(binding.declaration.path))));
  if(binding.declaration.owner?.name!=='@finds-team/frontend')externalDeclarations++;
  for(const context of binding.contexts){
    assert.equal(source.subarray(context.span.start,context.span.end).toString(),context.text);
    const operations=model.operations.filter(operation=>operation.span.start===context.span.start&&operation.span.end===context.span.end);
    assert.deepEqual(context.operations,operations.map(({id,kind,execution})=>({id,kind,execution})));
    assert.equal(context.status,operations.length===1?'exact':'open');if(context.status==='exact')exactContexts++;else openContexts++;
  }
}
assert.equal(exactContexts,bindings.summary.exactExpressionContexts);assert.equal(openContexts,bindings.summary.openExpressionContexts);assert.equal(externalDeclarations,bindings.summary.externalDeclarationBindings);
for(const row of report.rows.filter(row=>!row.outputByteIdentity))assert(bindings.open.some(item=>item.file===row.file&&item.reason==='installed-output-differs'));
assert.equal(bindings.summary.outputDriftRefusals,report.rows.length-matching);assert.equal(bindings.summary.findings,0);
const summary={sourcePaths:new Set(report.rows.map(row=>row.path)).size,distinctSourceContents:new Set(report.rows.map(row=>row.sourceSha256)).size,packageSourcePaths:report.summary.packageFiles,
  compilerRuns:report.rows.length,traceOnOffIdentityRuns:report.rows.length,emptyOperationRuns:report.rows.filter(row=>row.sourceOperations===0&&row.generatedOperations===0).length,
  installedOutputMatches:matching,installedOutputDrifts:report.rows.length-matching,sourceOperations,generatedOperations,matchingOutputSourceOperations:report.rows.filter(row=>row.outputByteIdentity).reduce((sum,row)=>sum+row.sourceOperations,0),
  contexts,bindings:bindings.bindings.length,externalDeclarations,exactExpressionContexts:exactContexts,openExpressionContexts:openContexts,unsupportedModeRefusals:2,findings:0};
const audit={authority:false,certification:false,auditedAt:new Date().toISOString(),inputs:[input,bindingsPath].map(path=>({path,sha256:hash(readFileSync(path))})),summary,
  limits:['Independent artifact audit checks reported declaration bytes and spans; TypeScript symbol acquisition is performed by the binding runner.','Compiler contexts remain statements about the pinned producer; neither output agreement nor a declaration establishes full build/runtime identity.','No causal diagnosis, new defect, warning precision or package-runtime closure follows from these counts.']};
writeFileSync(output,JSON.stringify(audit,null,2)+'\n');console.log(JSON.stringify(summary));
