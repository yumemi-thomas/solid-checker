// Try narrow getter evidence from the real installed compiler. No helper-name
// model, source-map interpolation, or fact-fork output equivalence is trusted.
import assert from 'node:assert/strict';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {hash,packageRoot,packageDigest} from './catalog.mjs';
import {byteSpan} from './compiler-jsx-resolution-v1.mjs';
import {jsxExpressions,singleReturnGetters,getterExpressionCopy,exactMappedPoint} from './compiler-expression-copy-v1.mjs';

const [inputArg,outArg]=process.argv.slice(2),input=resolve(inputArg),out=resolve(outArg);assert(!existsSync(out));
const report=JSON.parse(readFileSync(input));assert.equal(hash(readFileSync(report.inputs.runner.path)),report.inputs.runner.sha256);
const compilerRoot=report.inputs.installed.find(row=>row.name==='@solidjs/compiler').root;
for(const pin of report.inputs.installed)assert.equal(packageDigest(pin.root),pin.digest);
assert.equal(hash(readFileSync(report.inputs.loadedBinary.path)),report.inputs.loadedBinary.sha256);
assert(!process.env.SOLID_COMPILER_NATIVE&&!process.env.NAPI_RS_FORCE_WASI);
assert(!existsSync(join(compilerRoot,'compiler.node'))&&!existsSync(join(compilerRoot,'compiler.darwin-arm64.node')));
const require=createRequire(join(compilerRoot,'index.js')),compiler=require(join(compilerRoot,'index.js'));
const root=resolve('rust/target/real-package-hydration-native-v1/application'),tsRoot=packageRoot(root,'typescript'),ts=require(join(tsRoot,'lib/typescript.js'));
const mappingRoot=packageRoot(resolve('rust/target/app-import-metric/apps/helge-dev'),'@jridgewell/trace-mapping');
const {TraceMap,decodedMappings}=await import(pathToFileURL(join(mappingRoot,'dist/trace-mapping.mjs')));
const tools=[tsRoot,mappingRoot].map(root=>({root,digest:packageDigest(root)}));mkdirSync(out);const rows=[];
for(const row of report.rows){
  const artifact=JSON.parse(readFileSync(join(resolve(input,'..'),row.file))),sourceText=readFileSync(row.path,'utf8');assert.equal(hash(sourceText),row.sourceSha256);
  const value=compiler.transform(sourceText,{...artifact.request.options,filename:row.path,sourceMap:true});
  assert.equal(value.code,artifact.installed.code);const map=JSON.parse(value.map);
  assert.equal(map.sources.length,1);assert.equal(map.sources[0],row.path);assert.equal(map.sourcesContent[0],sourceText);
  const source=ts.createSourceFile(row.path,sourceText,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const generated=ts.createSourceFile(row.path+'.generated.ts',value.code,ts.ScriptTarget.Latest,true,ts.ScriptKind.TS);
  assert.equal(source.parseDiagnostics.length,0);assert.equal(generated.parseDiagnostics.length,0);
  const expressions=jsxExpressions(ts,source),getters=singleReturnGetters(ts,generated);
  const decoded=decodedMappings(new TraceMap(map)),evidence=[];let mappedStarts=0;
  const open=[];
  for(const getter of getters){
    const expression=getter.body.statements[0].expression,start=generated.getLineAndCharacterOfPosition(expression.getStart(generated));
    const mappedStart=exactMappedPoint(decoded,start.line,start.character);
    if(mappedStart&&expressions.filter(node=>{const point=source.getLineAndCharacterOfPosition(node.getStart(source));return point.line===mappedStart.line&&point.character===mappedStart.character;}).length===1)mappedStarts++;
    const proof=getterExpressionCopy(ts,source,generated,decoded,expressions,getter);
    if(proof.status!=='copy-witness'){open.push({generatedGetterSpan:byteSpan(generated,getter),reason:proof.reason});continue;}
    const original=proof.original;
    evidence.push({sourceSpan:byteSpan(source,original),generatedExpressionSpan:byteSpan(generated,expression),generatedGetterSpan:byteSpan(generated,getter),text:original.getText(source),
      grade:'exact-start-and-parsed-expression-copy',atomMappings:proof.atomMappings,endMapping:proof.endMapping,
      execution:'generated-getter-returns-expression-copy',lexicalBinding:'open',tracking:'unknown',owner:'unknown',getterInvocation:'open',runtimeProvider:'open',finding:false});
  }
  const file=row.file;writeFileSync(join(out,file),JSON.stringify({authority:false,certification:false,request:artifact.request,sourceSha256:row.sourceSha256,code:value.code,map,evidence,open},null,2)+'\n');
  rows.push({file,sourcePath:row.path,mode:row.generate,factForkOutputDrift:!row.outputByteIdentity,mapCodeByteIdentity:true,sourceContentIdentity:true,jsxExpressions:expressions.length,generatedSingleReturnGetters:getters.length,exactMappedStarts:mappedStarts,expressionCopyWitnesses:evidence.length,exactEndpointEvidence:evidence.filter(item=>item.endMapping==='exact').length,openGetters:getters.length-evidence.length});
}
const loaded=Object.keys(require.cache).filter(path=>path.endsWith('.node'));assert.deepEqual(loaded,[report.inputs.loadedBinary.path]);
for(const pin of report.inputs.installed)assert.equal(packageDigest(pin.root),pin.digest);for(const tool of tools)assert.equal(packageDigest(tool.root),tool.digest);
const summary={runs:rows.length,mapCodeByteIdentityRuns:rows.filter(row=>row.mapCodeByteIdentity).length,generatedSingleReturnGetters:rows.reduce((s,r)=>s+r.generatedSingleReturnGetters,0),exactMappedStarts:rows.reduce((s,r)=>s+r.exactMappedStarts,0),expressionCopyWitnesses:rows.reduce((s,r)=>s+r.expressionCopyWitnesses,0),exactEndpointEvidence:rows.reduce((s,r)=>s+r.exactEndpointEvidence,0),openGetters:rows.reduce((s,r)=>s+r.openGetters,0),copyWitnessesOnDriftingForkRuns:rows.filter(r=>r.factForkOutputDrift).reduce((s,r)=>s+r.expressionCopyWitnesses,0),findings:0};
writeFileSync(join(out,'results.json'),JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),summary,rows,
  inputs:{report:{path:input,sha256:hash(readFileSync(input))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},copyVerifier:{path:fileURLToPath(new URL('./compiler-expression-copy-v1.mjs',import.meta.url)),sha256:hash(readFileSync(new URL('./compiler-expression-copy-v1.mjs',import.meta.url)))},installed:report.inputs.installed,loadedBinary:report.inputs.loadedBinary,tools},
  limits:['Copy witnesses require exact start, identical expression text and parsed structure, and exact maps for every identifier/literal. End mappings remain separately graded; nearby points are refused.','This locates a copied expression in an emitted getter. Lexical binding, helper scheduling, tracking scope, callback invocation, runtime provider, defects and warnings remain open.','Source maps and positive syntax cannot establish a complete execution census. Unmatched getters remain open; generated helper callbacks are outside this experiment.']},null,2)+'\n');
console.log(JSON.stringify(summary));

