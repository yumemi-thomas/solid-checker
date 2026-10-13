// Join exact compiler JSX spans to real TypeScript declarations without guessing
// package dispatch or inferring when a component invokes a callback property.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {hash,packageRoot} from './catalog.mjs';

const [inputArg,outArg]=process.argv.slice(2),inputPath=resolve(inputArg),output=resolve(outArg),report=JSON.parse(readFileSync(inputPath));assert(!existsSync(output));
assert(report.finishedAt);assert.equal(hash(readFileSync(report.inputs.runner.path)),report.inputs.runner.sha256);
const programs=new Map();let ts;
for(const observationPin of report.inputs.observations){
  assert.equal(hash(readFileSync(observationPin.path)),observationPin.sha256);
  const observation=JSON.parse(readFileSync(observationPin.path)),run=JSON.parse(readFileSync(observation.inputs.run.path)),root=run.clone;
  const tsRoot=packageRoot(root,'typescript'),require=createRequire(join(tsRoot,'package.json'));ts=require(join(tsRoot,'lib/typescript.js'));
  const config=ts.readConfigFile(join(root,'tsconfig.json'),ts.sys.readFile);assert(!config.error);
  const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,root);assert.deepEqual(parsed.errors,[]);
  const program=ts.createProgram({rootNames:parsed.fileNames,options:parsed.options});assert.deepEqual(ts.getPreEmitDiagnostics(program),[]);
  programs.set(observation.variant,{program,checker:program.getTypeChecker(),root,typescript:{root:tsRoot,version:ts.version}});
}
function bytes(source,node){return {start:Buffer.byteLength(source.text.slice(0,node.getStart(source)),'utf8'),end:Buffer.byteLength(source.text.slice(0,node.end),'utf8')};}
function sameSpan(left,right){return left.start===right.start&&left.end===right.end;}
function ownership(path){for(let dir=dirname(path);;dir=dirname(dir)){const manifest=join(dir,'package.json');if(existsSync(manifest)){const value=JSON.parse(readFileSync(manifest));return {name:value.name,version:value.version,root:realpathSync(dir),manifestSha256:hash(readFileSync(manifest))};}if(dirname(dir)===dir)return null;}}
const bindings=[],open=[];
for(const row of report.rows){
  if(!row.outputByteIdentity){open.push({file:row.file,reason:'installed-output-differs'});continue;}
  const artifactPath=join(dirname(inputPath),row.file),artifact=JSON.parse(readFileSync(artifactPath)),{program,checker}=programs.get(row.variant);
  assert.equal(artifact.facts.output,artifact.installed.code);assert.equal(hash(readFileSync(row.path)),row.sourceSha256);
  const source=program.getSourceFile(row.path);
  if(!source){open.push({file:row.file,reason:'source-outside-configured-type-program'});continue;}
  assert.equal(hash(source.text),row.sourceSha256);
  const elements=[];function visit(node){if(ts.isJsxElement(node)||ts.isJsxSelfClosingElement(node))elements.push(node);ts.forEachChild(node,visit);}visit(source);
  const model=artifact.facts.executionMap.semanticModel;
  for(const generated of model.generatedOperations.filter(operation=>operation.kind==='component-invocation')){
    const candidates=elements.filter(node=>sameSpan(bytes(source,node),generated.sourceSpan));
    if(candidates.length!==1){open.push({file:row.file,operation:generated.id,reason:'jsx-span-not-unique'});continue;}
    const element=candidates[0],opening=ts.isJsxElement(element)?element.openingElement:element,tag=opening.tagName;
    let symbol=checker.getSymbolAtLocation(tag);const visited=new Set();
    while(symbol&&(symbol.flags&ts.SymbolFlags.Alias)){if(visited.has(symbol)){symbol=null;break;}visited.add(symbol);symbol=checker.getAliasedSymbol(symbol);}
    const declarations=symbol?.declarations??[];
    if(declarations.length!==1){open.push({file:row.file,operation:generated.id,reason:'declaration-not-unique'});continue;}
    const declaration=declarations[0],declSource=declaration.getSourceFile(),declSpan=bytes(declSource,declaration);
    const expressions=[];
    for(const attribute of opening.attributes.properties){
      if(ts.isJsxSpreadAttribute(attribute))expressions.push({role:'spread',node:attribute.expression});
      else if(attribute.initializer&&ts.isJsxExpression(attribute.initializer)&&attribute.initializer.expression)expressions.push({role:'property',node:attribute.initializer.expression});
    }
    if(ts.isJsxElement(element))for(const child of element.children){if(ts.isJsxExpression(child)&&child.expression)expressions.push({role:'child',node:child.expression});}
    const contexts=expressions.map(({role,node})=>{
      const span=bytes(source,node),operations=model.operations.filter(operation=>sameSpan(operation.span,span));
      return {role,span,text:Buffer.from(source.text).subarray(span.start,span.end).toString(),operations:operations.map(({id,kind,execution})=>({id,kind,execution})),status:operations.length===1?'exact':'open'};
    });
    const point=source.getLineAndCharacterOfPosition(tag.getStart(source));
    bindings.push({file:row.file,artifact:{path:artifactPath,sha256:hash(readFileSync(artifactPath))},source:{path:row.path,sha256:row.sourceSha256,span:generated.sourceSpan,line:point.line+1,column:point.character+1},mode:row.generate,
      generated,tag:{text:tag.getText(source),span:bytes(source,tag)},
      declaration:{path:declSource.fileName,span:declSpan,sourceSha256:hash(declSource.text),owner:ownership(declSource.fileName)},contexts,
      installedOutputAgreement:true,runtimeProvider:'open',callbackInvocation:'open',causalResponsibility:'open',finding:false});
  }
}
const summary={typedPrograms:programs.size,typingDiagnostics:0,bindings:bindings.length,externalDeclarationBindings:bindings.filter(row=>row.declaration.owner?.name!== '@finds-team/frontend').length,
  exactExpressionContexts:bindings.reduce((sum,row)=>sum+row.contexts.filter(context=>context.status==='exact').length,0),openExpressionContexts:bindings.reduce((sum,row)=>sum+row.contexts.filter(context=>context.status!=='exact').length,0),
  outputDriftRefusals:open.filter(row=>row.reason==='installed-output-differs').length,outsideTypeProgram:open.filter(row=>row.reason==='source-outside-configured-type-program').length,
  unresolvedDeclarations:open.filter(row=>row.reason==='declaration-not-unique').length,ambiguousJsxSpans:open.filter(row=>row.reason==='jsx-span-not-unique').length,findings:0};
const result={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:{report:{path:inputPath,sha256:hash(readFileSync(inputPath))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},programs:[...programs.values()].map(({root,typescript})=>({root,typescript}))},summary,bindings,open,
  limits:['Exact source span and unique declaration resolution only; no name-based package trust.','Source ownership and declarations do not prove the runtime provider or invocation of a callback property.','Per-file generated-output agreement does not bind the whole production build or compiler identity.','Source outside the configured TypeScript program and drifting compiler output remain open.','These execution-context notes are not new violations or warnings.']};
writeFileSync(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(summary));
