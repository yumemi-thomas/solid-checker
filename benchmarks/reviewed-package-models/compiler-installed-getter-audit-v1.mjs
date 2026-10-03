// Recompile with the pinned installed native artifact, then reconstruct copy
// evidence. The point-map verifier is shared and separately mutation-tested.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {hash,packageDigest,packageRoot} from './catalog.mjs';
import {byteSpan} from './compiler-jsx-resolution-v1.mjs';
import {jsxExpressions,singleReturnGetters,getterExpressionCopy} from './compiler-expression-copy-v1.mjs';
const [inputArg,outArg]=process.argv.slice(2),input=resolve(inputArg),out=resolve(outArg);assert(!existsSync(out));
const report=JSON.parse(readFileSync(input));assert.equal(report.authority,false);assert.equal(report.certification,false);
function pin(value){assert.equal(hash(readFileSync(value.path)),value.sha256);}
pin(report.inputs.report);pin(report.inputs.runner);pin(report.inputs.copyVerifier);pin(report.inputs.loadedBinary);
for(const value of [...report.inputs.installed,...report.inputs.tools])assert.equal(packageDigest(value.root),value.digest);
const compilerRoot=report.inputs.installed.find(value=>value.name==='@solidjs/compiler').root;
assert(!process.env.SOLID_COMPILER_NATIVE&&!process.env.NAPI_RS_FORCE_WASI);
assert(!existsSync(join(compilerRoot,'compiler.node'))&&!existsSync(join(compilerRoot,'compiler.darwin-arm64.node')));
const require=createRequire(join(compilerRoot,'index.js')),compiler=require(join(compilerRoot,'index.js'));
const tsRoot=packageRoot(resolve('rust/target/real-package-hydration-native-v1/application'),'typescript'),ts=require(join(tsRoot,'lib/typescript.js'));
const mappingRoot=packageRoot(resolve('rust/target/app-import-metric/apps/helge-dev'),'@jridgewell/trace-mapping');
const {TraceMap,decodedMappings}=await import(pathToFileURL(join(mappingRoot,'dist/trace-mapping.mjs')));
const original=JSON.parse(readFileSync(report.inputs.report.path));assert.equal(report.rows.length,original.rows.length);
let witnesses=0,open=0,getters=0,atoms=0,driftingWitnesses=0;
for(const [index,row] of report.rows.entries()){
  const artifact=JSON.parse(readFileSync(join(dirname(input),row.file))),originalRow=original.rows[index];assert.equal(row.file,originalRow.file);assert.equal(row.sourcePath,originalRow.path);
  assert.equal(artifact.authority,false);assert.equal(artifact.certification,false);
  const sourceText=readFileSync(row.sourcePath,'utf8');assert.equal(hash(sourceText),artifact.sourceSha256);assert.equal(artifact.sourceSha256,originalRow.sourceSha256);
  const raw=JSON.parse(readFileSync(join(dirname(report.inputs.report.path),row.file)));assert.deepEqual(artifact.request,raw.request);
  const fresh=compiler.transform(sourceText,{...artifact.request.options,filename:row.sourcePath,sourceMap:true});assert.equal(fresh.code,artifact.code);assert.equal(fresh.code,raw.installed.code);assert.deepEqual(JSON.parse(fresh.map),artifact.map);
  assert.deepEqual(artifact.map.sources,[row.sourcePath]);assert.deepEqual(artifact.map.sourcesContent,[sourceText]);
  const source=ts.createSourceFile(row.sourcePath,sourceText,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),generated=ts.createSourceFile('g.ts',artifact.code,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS),expressions=jsxExpressions(ts,source),generatedGetters=singleReturnGetters(ts,generated),decoded=decodedMappings(new TraceMap(artifact.map));
  const seen=new Set();assert.equal(row.generatedSingleReturnGetters,generatedGetters.length);assert.equal(row.jsxExpressions,expressions.length);
  for(const getter of generatedGetters){
    const span=byteSpan(generated,getter),proof=getterExpressionCopy(ts,source,generated,decoded,expressions,getter);
    if(proof.status==='copy-witness'){
      const matching=artifact.evidence.filter(value=>value.generatedGetterSpan.start===span.start&&value.generatedGetterSpan.end===span.end);assert.equal(matching.length,1);
      const value=matching[0];assert(!seen.has(span.start));seen.add(span.start);
      assert.deepEqual(value.sourceSpan,byteSpan(source,proof.original));assert.deepEqual(value.generatedExpressionSpan,byteSpan(generated,proof.expression));
      assert.equal(value.text,proof.original.getText(source));assert.deepEqual(value.atomMappings,proof.atomMappings);assert.equal(value.grade,'exact-start-and-parsed-expression-copy');assert.equal(value.endMapping,proof.endMapping);
      assert.equal(value.execution,'generated-getter-returns-expression-copy');assert.equal(value.lexicalBinding,'open');assert.equal(value.tracking,'unknown');assert.equal(value.owner,'unknown');assert.equal(value.getterInvocation,'open');assert.equal(value.runtimeProvider,'open');assert.equal(value.finding,false);
      witnesses++;atoms+=proof.atomMappings.length;if(row.factForkOutputDrift)driftingWitnesses++;
    }else{
      const matching=artifact.open.filter(value=>value.generatedGetterSpan.start===span.start&&value.generatedGetterSpan.end===span.end);assert.equal(matching.length,1);assert.equal(matching[0].reason,proof.reason);open++;
    }
  }
  assert.equal(row.expressionCopyWitnesses,artifact.evidence.length);assert.equal(row.openGetters,artifact.open.length);assert.equal(artifact.evidence.length+artifact.open.length,generatedGetters.length);
  assert.equal(row.factForkOutputDrift,!originalRow.outputByteIdentity);getters+=generatedGetters.length;
}
assert.deepEqual(Object.keys(require.cache).filter(path=>path.endsWith('.node')),[report.inputs.loadedBinary.path]);pin(report.inputs.loadedBinary);
assert.equal(witnesses,report.summary.expressionCopyWitnesses);assert.equal(open,report.summary.openGetters);assert.equal(getters,report.summary.generatedSingleReturnGetters);assert.equal(driftingWitnesses,report.summary.copyWitnessesOnDriftingForkRuns);assert.equal(report.summary.findings,0);
const summary={recompiledRuns:report.rows.length,exactMapAndCodeRuns:report.rows.length,expressionCopyWitnesses:witnesses,mappedAtoms:atoms,openGetters:open,copyWitnessesOnDriftingForkRuns:driftingWitnesses,findings:0};
writeFileSync(out,JSON.stringify({authority:false,certification:false,summary,inputs:[input,fileURLToPath(import.meta.url)].map(path=>({path,sha256:hash(readFileSync(path))})),
  limits:['The audit independently repeats the actual installed native compilation and map/code identity checks. Expression-copy reconstruction shares the separately tested verifier.','A copied expression and positive getter syntax do not establish lexical binding, tracking, owner, runtime invocation, completeness or a defect.']},null,2)+'\n');console.log(JSON.stringify(summary));
