// Characterize emission drift for review. This never authorizes a binding.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {hash,packageRoot} from './catalog.mjs';
const [inputArg,outArg]=process.argv.slice(2),input=resolve(inputArg),out=resolve(outArg);assert(!existsSync(out));
const report=JSON.parse(readFileSync(input)),tsRoot=packageRoot(resolve('rust/target/real-package-hydration-native-v1/application'),'typescript');
const ts=createRequire(join(tsRoot,'package.json'))(join(tsRoot,'lib/typescript.js'));
function tree(node,staticGetterKeys){
  if(staticGetterKeys&&ts.isComputedPropertyName(node)&&ts.isGetAccessorDeclaration(node.parent)&&ts.isStringLiteral(node.expression))return tree(node.expression,false);
  const children=[];ts.forEachChild(node,child=>{children.push(tree(child,staticGetterKeys));});
  const value=ts.isIdentifier(node)?node.escapedText:ts.isLiteralExpression(node)?node.text:null;
  return {kind:node.kind,value,children};
}
const rows=[];
for(const row of report.rows.filter(row=>!row.outputByteIdentity)){
  const artifact=JSON.parse(readFileSync(join(dirname(input),row.file)));
  assert.notEqual(artifact.facts.output,artifact.installed.code);
  const facts=ts.createSourceFile('output.js',artifact.facts.output,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),installed=ts.createSourceFile('output.js',artifact.installed.code,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS);
  const syntaxClean=facts.parseDiagnostics.length===0&&installed.parseDiagnostics.length===0;
  const exactTree=syntaxClean&&JSON.stringify(tree(facts,false))===JSON.stringify(tree(installed,false));
  const staticGetterTree=syntaxClean&&JSON.stringify(tree(facts,true))===JSON.stringify(tree(installed,true));
  rows.push({file:row.file,sourceSha256:row.sourceSha256,syntaxClean,category:exactTree?'same-syntax-tree-without-trivia':staticGetterTree?'static-getter-key-emission':'other-syntax-differences',outputByteIdentity:false,bindingAllowed:false});
}
const summary={drifts:rows.length,categories:Object.fromEntries([...new Set(rows.map(row=>row.category))].map(kind=>[kind,rows.filter(row=>row.category===kind).length])),admittedBindings:0};
writeFileSync(out,JSON.stringify({authority:false,certification:false,summary,rows,inputs:[input,fileURLToPath(import.meta.url)].map(path=>({path,sha256:hash(readFileSync(path))})),
  limits:['Syntax comparison omits source positions and trivia. It is a review aid, not a proof of execution or compiler identity.','Static getter key comparison does not weaken exact emitted-byte admission; every listed source/mode remains open.']},null,2)+'\n');
console.log(JSON.stringify(summary));
