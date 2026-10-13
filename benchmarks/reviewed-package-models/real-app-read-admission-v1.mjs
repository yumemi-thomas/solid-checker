// Real source demand and current exact-symbol enrollment, never a defect verdict.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {performance} from 'node:perf_hooks';
import {ts} from './lower.mjs';
import {hash,read,nativeRuntimeRoots} from './catalog.mjs';
import {packageSourceSession} from './package-source-session-v2.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v2.mjs';
import {callbackSlotSites} from './callback-slot-sites-v2.mjs';
const [priorArg,sealArg,outArg]=process.argv.slice(2),priorPath=resolve(priorArg),sealPath=resolve(sealArg),out=resolve(outArg);
assert(!existsSync(out));const prior=read(priorPath),seal=read(sealPath);
const authenticate=()=>{for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256,pin.path);};authenticate();
const report={authority:false,certification:false,startedAt:new Date().toISOString(),scope:'exact published declaration identities at calls/JSX plus current source admission over retained configurations; no runtime reachability, warning precision or app-correctness claim',inputs:[priorPath,sealPath,new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))})),results:[]};
const save=()=>writeFileSync(out,JSON.stringify(report,null,2)+'\n');save();
const nativePackages=new Set(['solid-js','@solidjs/signals','@solidjs/web','typescript']);
for(const original of prior.results){
  const row={app:original.app,root:original.root,configPath:original.configPath,files:[],ownerMetadata:[],authority:false,certification:false};report.results.push(row);let session;
  const started=performance.now();
  try{await(async()=>{
    row.nativeRuntime=nativeRuntimeRoots(row.root).map(path=>({path,name:read(join(path,'package.json')).name,version:read(join(path,'package.json')).version}));
    assert(row.nativeRuntime.every(runtime=>runtime.version==='2.0.0-rc.9'),'Installed native runtime differs from the audited rc.9');
    const config=ts.readConfigFile(row.configPath,ts.sys.readFile);assert(!config.error);
    const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,row.root,{},row.configPath);
    const anchor=parsed.fileNames.find(path=>path.startsWith(row.root+'/src/')&&!path.endsWith('.d.ts'));assert(anchor,'No configured source anchor');
    session=packageSourceSession(row.root,{configPath:row.configPath});const state=session.get(anchor,readFileSync(anchor,'utf8')),program=state.publishedProgram,checker=program.getTypeChecker();
    row.publishedTypingErrors=state.errors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
    row.sourceFactDiagnostics=state.sourceErrors.map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
    assert.deepEqual(row.publishedTypingErrors,original.publishedTypingErrors);
    const owners=new Map(),metadata=new Map();
    function owner(file){if(owners.has(file))return owners.get(file);let result=null;for(let dir=dirname(file);;dir=dirname(dir)){const path=join(dir,'package.json');if(existsSync(path)){const bytes=readFileSync(path),pkg=JSON.parse(bytes);if(pkg.name&&pkg.version){result={root:realpathSync(dir),package:pkg.name,version:pkg.version,metadata:path,metadataSha256:hash(bytes)};metadata.set(path,result);}break;}if(dirname(dir)===dir)break;}owners.set(file,result);return result;}
    const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
    function unwrapped(node){while(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)||ts.isTypeAssertionExpression(node))node=node.expression;return node;}
    for(const source of program.getSourceFiles().filter(source=>!source.isDeclarationFile&&source.fileName.startsWith(row.root+'/src/')&&/\.[jt]sx?$/.test(source.fileName))){
      const file={path:source.fileName,sourceSha256:hash(source.text),callExpressions:0,jsxElements:0,unresolvedCalls:0,unresolvedJSX:0,externalReferences:[]};
      function reference(node,callee,kind){
        callee=unwrapped(callee);const location=ts.isIdentifier(callee)?callee:ts.isPropertyAccessExpression(callee)?callee.name:null;
        const symbol=location&&target(checker.getSymbolAtLocation(location));
        if(!symbol?.declarations?.length){if(kind==='call')file.unresolvedCalls++;else file.unresolvedJSX++;return;}
        const declarations=symbol.declarations.map(declaration=>({path:declaration.getSourceFile().fileName,start:declaration.getStart(),end:declaration.end,sha256:hash(declaration.getSourceFile().text)}));
        const providers=[...new Map(declarations.map(declaration=>owner(declaration.path)).filter(pkg=>pkg&&!nativePackages.has(pkg.package)&&!row.root.startsWith(pkg.root+'/')&&pkg.root!==row.root).map(pkg=>[pkg.root,pkg])).values()];
        if(!providers.length)return;
        file.externalReferences.push({kind,start:node.getStart(source),end:node.end,calleeStart:callee.getStart(source),calleeEnd:callee.end,name:symbol.getName(),providers,declarations,optional:!!node.questionDotToken,dispatch:'open',authority:false,certification:false});
      }
      function visit(node){if(ts.isCallExpression(node)){file.callExpressions++;reference(node,node.expression,'call');}else if(ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node)){file.jsxElements++;reference(node,node.tagName,'jsx');}ts.forEachChild(node,visit);}visit(source);
      if(row.publishedTypingErrors.length){file.candidates=[];file.asyncFunctions=0;file.callbackEntries=0;file.callbackAllocations=0;file.callbackInvocations=0;file.admission='closed-public-typing-errors';}
      else{
        const sourceView=state.program.getSourceFile(source.fileName);assert(sourceView&&sourceView.text===source.text);
        const sites=nativeReadSites(state.program,sourceView),helpers=asyncContinuationSites(state.program,sourceView),slots=callbackSlotSites(state.program,sourceView);
        file.candidates=sites.sites;file.asyncFunctions=helpers.functions.length;file.callbackEntries=slots.callbacks.length;file.callbackAllocations=slots.allocations.length;file.callbackInvocations=slots.invocations.length;
        file.openCounts={reads:sites.open.length,helpers:helpers.open.length,callbacks:slots.open.length};file.admission='current-source-facts';
      }
      row.files.push(file);
    }
    row.ownerMetadata=[...metadata.values()];row.revision=state.revision;row.inputManifest=session.inputs();row.runtimeSources=session.runtimeSources();row.sourceOpen=session.sourceOpen();row.stats={...session.stats};
    row.summary={files:row.files.length,calls:row.files.reduce((sum,file)=>sum+file.callExpressions,0),externalCalls:row.files.flatMap(file=>file.externalReferences).filter(reference=>reference.kind==='call').length,externalJSX:row.files.flatMap(file=>file.externalReferences).filter(reference=>reference.kind==='jsx').length,unresolvedCalls:row.files.reduce((sum,file)=>sum+file.unresolvedCalls,0),candidates:row.files.reduce((sum,file)=>sum+file.candidates.length,0),asyncFunctions:row.files.reduce((sum,file)=>sum+file.asyncFunctions,0),callbackEntries:row.files.reduce((sum,file)=>sum+file.callbackEntries,0),runtimeModules:row.runtimeSources.length,typingClosed:row.publishedTypingErrors.length>0};
  })();}
  catch(error){row.refused={message:error.message,stack:error.stack};}
  finally{session?.invalidate();row.elapsedMs=performance.now()-started;await new Promise(resolve=>setImmediate(resolve));globalThis.gc?.();row.memoryAfterCollection=process.memoryUsage();save();}
}
authenticate();for(const pin of report.inputs)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
for(const row of report.results){for(const pin of row.ownerMetadata)assert.equal(hash(readFileSync(pin.metadata)),pin.metadataSha256);for(const file of row.files)assert.equal(hash(readFileSync(file.path)),file.sourceSha256);}
report.finishedAt=new Date().toISOString();report.summary={projects:report.results.length,refused:report.results.filter(row=>row.refused).length,typingClean:report.results.filter(row=>row.publishedTypingErrors?.length===0&&!row.refused).length,typingClosed:report.results.filter(row=>row.publishedTypingErrors?.length>0&&!row.refused).length,files:report.results.reduce((sum,row)=>sum+row.files.length,0),externalCalls:report.results.reduce((sum,row)=>sum+(row.summary?.externalCalls??0),0),externalJSX:report.results.reduce((sum,row)=>sum+(row.summary?.externalJSX??0),0),candidateSites:report.results.reduce((sum,row)=>sum+(row.summary?.candidates??0),0)};save();console.log(JSON.stringify(report.summary));
