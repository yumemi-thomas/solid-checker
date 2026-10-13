import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {hash,packageRoot} from './catalog.mjs';
import {jsxExpressions,singleReturnGetters,getterExpressionCopy} from './compiler-expression-copy-v1.mjs';
import {byteSpan} from './compiler-jsx-resolution-v1.mjs';
const [inputArg,outArg]=process.argv.slice(2),input=resolve(inputArg),out=resolve(outArg);assert(!existsSync(out));
const report=JSON.parse(readFileSync(input)),root=packageRoot(resolve('rust/target/real-package-hydration-native-v1/application'),'typescript'),ts=createRequire(join(root,'package.json'))(join(root,'lib/typescript.js'));
const {TraceMap,decodedMappings}=await import(pathToFileURL(join(packageRoot(resolve('rust/target/app-import-metric/apps/helge-dev'),'@jridgewell/trace-mapping'),'dist/trace-mapping.mjs')));
let sample;
for(const row of report.rows){
  const artifact=JSON.parse(readFileSync(join(dirname(input),row.file)));
  const source=ts.createSourceFile(row.sourcePath,artifact.map.sourcesContent[0],ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),generated=ts.createSourceFile('g.ts',artifact.code,ts.ScriptTarget.Latest,true),decoded=decodedMappings(new TraceMap(artifact.map)),expressions=jsxExpressions(ts,source);
  const evidence=artifact.evidence.find(item=>item.atomMappings.length>=2&&expressions.filter(node=>node.getText(source)===item.text).length>1);
  if(!evidence)continue;
  const getter=singleReturnGetters(ts,generated).find(node=>byteSpan(generated,node).start===evidence.generatedGetterSpan.start);
  sample={row,artifact,source,generated,decoded,expressions,getter,evidence};break;
}
assert(sample,'Need a real copied expression with repeated source text and multiple mapped atoms');
const {source,generated,decoded,expressions,getter,evidence}=sample;
const verify=(mapping=decoded,output=generated,node=getter)=>getterExpressionCopy(ts,source,output,mapping,expressions,node);
const positive=verify();assert.equal(positive.status,'copy-witness');assert.deepEqual(byteSpan(source,positive.original),evidence.sourceSpan);assert.equal(positive.endMapping,'unobserved');
const start=generated.getLineAndCharacterOfPosition(getter.body.statements[0].expression.getStart(generated));
function segment(mapping,point){return mapping[point.line].find(row=>row[0]===point.character);}
function refusal(name,change){const mapping=structuredClone(decoded);change(mapping);assert.equal(verify(mapping).status,'open',name);return name;}
const negatives=[
  refusal('nearby-generated-point',mapping=>{segment(mapping,start)[0]--;}),
  refusal('nearby-source-point',mapping=>{segment(mapping,start)[3]++;}),
  refusal('unmapped-start',mapping=>{segment(mapping,start).splice(1);}),
  refusal('duplicate-start',mapping=>{mapping[start.line].push([...segment(mapping,start)]);}),
  refusal('wrong-source-index',mapping=>{segment(mapping,start)[1]=1;})
];
const separate=evidence.atomMappings.find(atom=>atom.generatedPoint.line!==start.line||atom.generatedPoint.character!==start.character);assert(separate);
negatives.push(refusal('wrong-descendant-point',mapping=>{segment(mapping,separate.generatedPoint)[3]++;}));
const expression=getter.body.statements[0].expression,changedText=generated.text.slice(0,expression.getStart(generated))+'x'.repeat(expression.getWidth(generated))+generated.text.slice(expression.end);
const changed=ts.createSourceFile('changed.ts',changedText,ts.ScriptTarget.Latest,true),changedGetter=singleReturnGetters(ts,changed).find(node=>byteSpan(changed,node).start===evidence.generatedGetterSpan.start);
assert(changedGetter);assert.equal(verify(decoded,changed,changedGetter).status,'open');negatives.push('changed-expression-copy');
writeFileSync(out,JSON.stringify({authority:false,certification:false,passed:9,positive:{file:sample.row.file,text:evidence.text,sourceSpan:evidence.sourceSpan,duplicateTextUses:expressions.filter(node=>node.getText(source)===evidence.text).length,grade:positive.status,endMapping:positive.endMapping},refused:negatives,
  inputs:[input,fileURLToPath(import.meta.url),fileURLToPath(new URL('./compiler-expression-copy-v1.mjs',import.meta.url))].map(path=>({path,sha256:hash(readFileSync(path))}))},null,2)+'\n');console.log(JSON.stringify({passed:9,refused:negatives.length}));
