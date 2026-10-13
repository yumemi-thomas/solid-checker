// Join exact compiler JSX spans to real TypeScript declarations without guessing
// package dispatch or inferring when a component invokes a callback property.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {existsSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {hash,packageRoot,packageDigest} from './catalog.mjs';

import {byteSpan as bytes,resolveJsxDeclaration} from './compiler-jsx-resolution-v1.mjs';

const [inputArg,outArg,variantArg]=process.argv.slice(2),inputPath=resolve(inputArg),output=resolve(outArg),report=JSON.parse(readFileSync(inputPath));assert(!existsSync(output));
assert(report.finishedAt);assert.equal(hash(readFileSync(report.inputs.runner.path)),report.inputs.runner.sha256);

if(!variantArg){
  const workers=[];
  for(const variant of ['candidate','native','package']){
    const path=output+'.'+variant+'.json';
    assert(!existsSync(path));
    const child=spawnSync(process.execPath,['--max-old-space-size=2048',fileURLToPath(import.meta.url),inputPath,path,variant],{encoding:'utf8',timeout:60000,maxBuffer:32*1024*1024});
    const processResult={exitCode:child.status,signal:child.signal,stdout:child.stdout,stderr:child.stderr,error:child.error?.message??null};
    writeFileSync(path+'.process.json',JSON.stringify(processResult,null,2)+'\n');
    assert.equal(child.status,0,JSON.stringify(processResult));
    const report=JSON.parse(readFileSync(path));workers.push({path,sha256:hash(readFileSync(path)),report});
  }
  const summary=Object.fromEntries(Object.keys(workers[0].report.summary).map(key=>[key,workers.reduce((sum,worker)=>sum+worker.report.summary[key],0)]));
  const result={authority:false,certification:false,finishedAt:new Date().toISOString(),
    inputs:{report:{path:inputPath,sha256:hash(readFileSync(inputPath))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},
      resolver:workers[0].report.inputs.resolver,programs:workers.flatMap(worker=>worker.report.inputs.programs),workers:workers.map(({path,sha256})=>({path,sha256}))},
    summary,decisions:workers.flatMap(worker=>worker.report.decisions),bindings:workers.flatMap(worker=>worker.report.bindings),open:workers.flatMap(worker=>worker.report.open),
    limits:workers[0].report.limits.concat(['One configured project or bounded implementation program per child, with a 2 GiB V8 heap cap and a 60-second timeout. Prior clean CLI typing runs are authenticated and reused; project-wide diagnostics are not recomputed.'])};
  writeFileSync(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(summary));process.exit(0);
}
assert(['candidate','native','package'].includes(variantArg));
const programs=new Map();let ts;
for(const observationPin of report.inputs.observations){
  assert.equal(hash(readFileSync(observationPin.path)),observationPin.sha256);
  const observation=JSON.parse(readFileSync(observationPin.path));if(observation.variant!==(variantArg==='package'?'candidate':variantArg))continue;
  assert.equal(hash(readFileSync(observation.inputs.run.path)),observation.inputs.run.sha256);
  const run=JSON.parse(readFileSync(observation.inputs.run.path)),root=run.clone;assert.equal(run.typing.exitCode,0);assert(run.originalAndCopiedInputsUnchanged);
  const tsRoot=packageRoot(root,'typescript'),require=createRequire(join(tsRoot,'package.json'));ts=require(join(tsRoot,'lib/typescript.js'));
  const config=ts.readConfigFile(join(root,'tsconfig.json'),ts.sys.readFile);assert(!config.error);
  const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,root);assert.deepEqual(parsed.errors,[]);
  const roots=variantArg==='package'?report.rows.filter(row=>JSON.parse(readFileSync(join(dirname(inputPath),row.file))).input.owner.role==='package-artifact').map(row=>row.path):parsed.fileNames;
  const rootNames=[...new Set(roots)];
  const options=variantArg==='package'?{allowJs:true,checkJs:false,noEmit:true,jsx:ts.JsxEmit.Preserve,jsxImportSource:'@solidjs/web',module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,target:ts.ScriptTarget.ES2022,skipLibCheck:true}:parsed.options;
  const program=ts.createProgram({rootNames,options});
  const kind=variantArg==='package'?'package-implementation':'checked-application';
  programs.set(variantArg,{program,checker:program.getTypeChecker(),root,kind,rootNames,options,typescript:{root:tsRoot,version:ts.version,digest:packageDigest(tsRoot)}});
}
function sameSpan(left,right){return left.start===right.start&&left.end===right.end;}
function ownership(path){for(let dir=dirname(path);;dir=dirname(dir)){const manifest=join(dir,'package.json');if(existsSync(manifest)){const value=JSON.parse(readFileSync(manifest));return {name:value.name,version:value.version,root:realpathSync(dir),manifestSha256:hash(readFileSync(manifest))};}if(dirname(dir)===dir)return null;}}
const bindings=[],open=[],decisions=[];
for(const row of report.rows){
  const artifactPath=join(dirname(inputPath),row.file),artifact=JSON.parse(readFileSync(artifactPath));
  const packageSource=artifact.input.owner.role==='package-artifact';
  if(variantArg==='package'?!packageSource:(packageSource||row.variant!==variantArg))continue;
  const decision={file:row.file,worker:variantArg,status:'open'};decisions.push(decision);
  if(!row.outputByteIdentity){decision.reason='installed-output-differs';open.push({file:row.file,reason:decision.reason});continue;}
  const {program,checker,kind}=programs.get(variantArg);
  assert.equal(artifact.facts.output,artifact.installed.code);assert.equal(hash(readFileSync(row.path)),row.sourceSha256);
  const source=program.getSourceFile(row.path);
  if(!source){decision.reason='source-outside-configured-type-program';open.push({file:row.file,reason:decision.reason});continue;}
  decision.status='source-bound';decision.programKind=kind;
  assert.equal(hash(source.text),row.sourceSha256);
  const elements=[];function visit(node){if(ts.isJsxElement(node)||ts.isJsxSelfClosingElement(node))elements.push(node);ts.forEachChild(node,visit);}visit(source);
  const model=artifact.facts.executionMap.semanticModel;
  for(const generated of model.generatedOperations.filter(operation=>operation.kind==='component-invocation')){
    const candidates=elements.filter(node=>sameSpan(bytes(source,node),generated.sourceSpan));
    if(candidates.length!==1){open.push({file:row.file,operation:generated.id,reason:'jsx-span-not-unique'});continue;}
    const element=candidates[0],opening=ts.isJsxElement(element)?element.openingElement:element,tag=opening.tagName;
    const resolved=resolveJsxDeclaration(ts,checker,opening,{checkedApplication:kind==='checked-application'});
    if(resolved.status!=='exact'){open.push({file:row.file,operation:generated.id,reason:resolved.reason});continue;}
    const declaration=resolved.declaration,declSource=declaration?.getSourceFile(),declSpan=declaration?bytes(declSource,declaration):null;
    const resolution={kind:resolved.kind,programKind:kind,symbolName:resolved.symbol.getName(),declarationCount:resolved.declarations.length,
      candidates:resolved.declarations.map(node=>({path:node.getSourceFile().fileName,span:bytes(node.getSourceFile(),node),sourceSha256:hash(node.getSourceFile().text)}))};
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
      resolution,declaration:declaration?{path:declSource.fileName,span:declSpan,sourceSha256:hash(declSource.text),owner:ownership(declSource.fileName)}:null,
      declarationSet:declaration?null:{members:resolution.candidates,owner:ownership(resolution.candidates[0].path),selectedOverload:null},contexts,
      installedOutputAgreement:true,runtimeProvider:'open',callbackInvocation:'open',causalResponsibility:'open',finding:false});
  }
}
const summary={typedPrograms:variantArg==='package'?0:programs.size,implementationPrograms:variantArg==='package'?programs.size:0,reusedCleanTypingRuns:variantArg==='package'?0:programs.size,sourceDecisions:decisions.length,overloadBindings:bindings.filter(row=>row.resolution.kind==='checked-overload-signature').length,overloadFamilyBindings:bindings.filter(row=>row.resolution.kind==='declared-overload-family').length,newTypingDiagnosticRuns:0,bindings:bindings.length,externalDeclarationBindings:bindings.filter(row=>(row.declaration?.owner??row.declarationSet?.owner)?.name!== '@finds-team/frontend').length,
  exactExpressionContexts:bindings.reduce((sum,row)=>sum+row.contexts.filter(context=>context.status==='exact').length,0),openExpressionContexts:bindings.reduce((sum,row)=>sum+row.contexts.filter(context=>context.status!=='exact').length,0),
  outputDriftRefusals:open.filter(row=>row.reason==='installed-output-differs').length,outsideTypeProgram:open.filter(row=>row.reason==='source-outside-configured-type-program').length,
  unresolvedDeclarations:open.filter(row=>row.reason==='declaration-not-unique').length,ambiguousJsxSpans:open.filter(row=>row.reason==='jsx-span-not-unique').length,findings:0};
const result={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:{report:{path:inputPath,sha256:hash(readFileSync(inputPath))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},resolver:{path:fileURLToPath(new URL('./compiler-jsx-resolution-v1.mjs',import.meta.url)),sha256:hash(readFileSync(new URL('./compiler-jsx-resolution-v1.mjs',import.meta.url)))},programs:[...programs.values()].map(({root,typescript,kind,rootNames,options})=>({root,typescript,kind,rootNames,options}))},summary,bindings,open,decisions,
  limits:['Exact source spans and symbol declarations; checked overload selection must be a member of that exact symbol.','Source ownership and declarations do not prove the runtime provider or invocation of a callback property.','Per-file generated-output agreement does not bind the whole production build or compiler identity.','A separate bounded published-implementation program adds exact declaration locations; checkJs:false does not establish clean package typing. Drifting output remains open.','These execution-context notes are not new violations or warnings.']};
writeFileSync(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(summary));
