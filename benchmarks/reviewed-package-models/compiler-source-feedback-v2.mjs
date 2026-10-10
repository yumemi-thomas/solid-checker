// Measure compiler execution facts on unchanged sources in the real browser bundle.
// Try a cached newer fact producer without changing production compiler pins.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {closurePins,hash,packageDigest,packageRoot} from './catalog.mjs';

const [outArg]=process.argv.slice(2),output=resolve(outArg);assert(!existsSync(output));
const sealPath=resolve('rust/target/compiler-source-handoff-freeze-v1.json'),seal=JSON.parse(readFileSync(sealPath));
for(const pin of seal.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const observationPaths=['rust/target/real-package-hydration-observed-candidate-v2/results.json','rust/target/real-package-hydration-observed-native-v1/results.json'].map(path=>resolve(path));
const observations=observationPaths.map(path=>JSON.parse(readFileSync(path)));
const inputs=new Map();
for(const report of observations){
  assert(report.finishedAt&&!report.failure&&report.clientByteIdentity.matched);
  assert.equal(hash(readFileSync(report.inputs.run.path)),report.inputs.run.sha256);
  for(const source of report.map.sources){
    if(!/\.(jsx|tsx)$/.test(source.path))continue;
    assert.equal(source.status,'exact');assert.equal(hash(readFileSync(source.path)),source.sourceSha256);
    inputs.set(source.path,{...source,variant:report.variant});
  }
}
const app=JSON.parse(readFileSync(observations[0].inputs.run.path)).clone;
const wrapper=packageRoot(app,'vite-plugin-solid'),pluginRoot=packageRoot(wrapper,'@solidjs/vite-plugin'),compilerRoot=packageRoot(pluginRoot,'@solidjs/compiler');
assert.equal(process.platform,'darwin');assert.equal(process.arch,'arm64');
assert(!process.env.SOLID_COMPILER_NATIVE&&!process.env.NAPI_RS_FORCE_WASI);
assert(!existsSync(join(compilerRoot,'compiler.node'))&&!existsSync(join(compilerRoot,'compiler.darwin-arm64.node')));
const nativeRoot=packageRoot(compilerRoot,'@solidjs/compiler-darwin-arm64');
const installedInputs=[pluginRoot,compilerRoot,nativeRoot].map(root=>({root,name:JSON.parse(readFileSync(join(root,'package.json'))).name,version:JSON.parse(readFileSync(join(root,'package.json'))).version,digest:packageDigest(root)}));
const require=createRequire(join(compilerRoot,'index.js')),compiler=require(join(compilerRoot,'index.js'));
const probe=resolve('rust/target/debug/debug/research-compiler-facts-probe-v2'),probeSha256=hash(readFileSync(probe));
const builtIns=['For','Show','Switch','Match','Loading','Reveal','Portal','Repeat','Dynamic','Errored'].sort();
const requests=[...inputs.values()].flatMap(source=>['dom','ssr'].map(generate=>({path:source.path,options:{moduleName:'@solidjs/web',generate,hydratable:true,dev:false,builtIns}})));
const boundaryRequests=['universal','dynamic'].map(generate=>({path:requests[0].path,options:{...requests[0].options,generate}}));
mkdirSync(output,{recursive:true});writeFileSync(join(output,'requests.json'),JSON.stringify([...requests,...boundaryRequests],null,2)+'\n');
const child=spawnSync(probe,[],{input:JSON.stringify([...requests,...boundaryRequests]),encoding:'utf8',timeout:60000,maxBuffer:64*1024*1024});
writeFileSync(join(output,'probe-stderr.log'),child.stderr??'');assert.equal(child.status,0,child.stderr);
writeFileSync(join(output,'probe.json'),child.stdout);const results=JSON.parse(child.stdout);assert.equal(results.length,requests.length+2);
const rows=[];
for(const [index,request] of requests.entries()){
  const facts=results[index],input=inputs.get(request.path),source=readFileSync(request.path,'utf8');
  assert.equal(facts.path,request.path);assert.deepEqual(facts.options,request.options);
  let installed;try{const value=compiler.transform(source,{...request.options,filename:request.path,sourceMap:false});installed={status:'compiled',code:value.code,map:value.map??null};}catch(error){installed={status:'refused',error:error.message};}
  const match=facts.status==='facts'&&installed.status==='compiled'&&facts.output===installed.code;
  const file=`${String(index).padStart(3,'0')}-${request.options.generate}.json`;writeFileSync(join(output,file),JSON.stringify({authority:false,certification:false,request,input,facts,installed},null,2)+'\n');
  const model=facts.executionMap?.semanticModel,operations=model?.operations??[],generated=model?.generatedOperations??[];
  rows.push({file,path:request.path,variant:input.variant,owner:input.owner.name,generate:request.options.generate,sourceSha256:input.sourceSha256,
    factStatus:facts.status,error:facts.error??null,installedStatus:installed.status,installedError:installed.error??null,outputByteIdentity:match,
    traceOnOffOutputIdentity:facts.traceOnOffOutputIdentity??false,sourceOperations:operations.length,generatedOperations:generated.length,
    sourceOperationsComplete:model?.sourceOperationsComplete??null,generatedOperationsComplete:model?.generatedOperationsComplete??null,
    dispositions:Object.fromEntries([...new Set(operations.map(row=>row.execution.disposition))].map(kind=>[kind,operations.filter(row=>row.execution.disposition===kind).length])),
    generatedKinds:Object.fromEntries([...new Set(generated.map(row=>row.kind))].map(kind=>[kind,generated.filter(row=>row.kind===kind).length])),
    runtimeFactBinding:'open',causalResponsibility:'open'});
}
const refusals=results.slice(requests.length);assert(refusals.every(row=>row.status==='refused'));
const loadedBinaries=Object.keys(require.cache).filter(path=>path.endsWith('.node'));assert.equal(loadedBinaries.length,1);assert(loadedBinaries[0].startsWith(nativeRoot+'/'));
for(const input of inputs.values())assert.equal(hash(readFileSync(input.path)),input.sourceSha256);
for(const input of installedInputs)assert.equal(packageDigest(input.root),input.digest);
assert.equal(hash(readFileSync(probe)),probeSha256);
const summary={files:inputs.size,packageFiles:[...inputs.values()].filter(row=>row.owner.role==='package-artifact').length,requests:rows.length,
  factRuns:rows.filter(row=>row.factStatus==='facts').length,factRefusals:rows.filter(row=>row.factStatus==='refused').length,
  installedRuns:rows.filter(row=>row.installedStatus==='compiled').length,outputMatches:rows.filter(row=>row.outputByteIdentity).length,
  sourceOperations:rows.reduce((sum,row)=>sum+row.sourceOperations,0),generatedOperations:rows.reduce((sum,row)=>sum+row.generatedOperations,0),unsupportedModesRefused:refusals.length};
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'Unchanged real bundle source, separate DOM and SSR compiler experiments. Source facts describe the pinned producer. Cross-compiler output identity is measured per exact source and configuration; runtime binding, package scheduling and causal diagnosis remain open.',
  inputs:{runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},seal:{path:sealPath,sha256:hash(readFileSync(sealPath))},
    observations:observationPaths.map(path=>({path,sha256:hash(readFileSync(path))})),probe:{path:probe,sha256:probeSha256},probeSource:['Cargo.toml','Cargo.lock','src/main.rs','adapter/Cargo.toml','adapter/src/lib.rs','adapter-identity.json'].map(path=>{const full=resolve('benchmarks/reviewed-package-models/compiler-facts-probe-v2',path);return {path:full,sha256:hash(readFileSync(full))};}),
    compilerIdentity:{path:resolve('benchmarks/reviewed-package-models/compiler-facts-probe-v2/adapter-identity.json'),sha256:hash(readFileSync('benchmarks/reviewed-package-models/compiler-facts-probe-v2/adapter-identity.json'))},
    installed:installedInputs,loadedBinary:{path:loadedBinaries[0],sha256:hash(readFileSync(loadedBinaries[0]))}},summary,rows,refusals,
  limits:['Output-neutral facts can classify compiler-controlled JSX execution; arbitrary package helper behavior remains outside this fact domain.','The installed compiler and pinned fact producer have distinct identities; this source experiment does not bind the full production build.','Generated operation enumeration remains partial, even when every source operation reconciles.','Fact collection and output agreement are not new defect detections or proof that the integration is correct.']};
writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(summary));

