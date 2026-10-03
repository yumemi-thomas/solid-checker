// Independent input-revision, frame and behavior audit. No detector imports.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync,statSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash,read,closurePins} from './catalog.mjs';
import {ts} from './lower.mjs';
import {auditAsyncConstant} from './async-constant-audit-v1.mjs';
import {auditConstantReturnedData} from './constant-returned-data-audit-v1.mjs';
import {auditCallbackSlot} from './callback-slot-audit-v4.mjs';
const [caseArg,readArg,plainArg,outArg]=process.argv.slice(2),casePath=resolve(caseArg),readPath=resolve(readArg),plainPath=resolve(plainArg),out=resolve(outArg);
assert(!existsSync(out));const cases=(await import(pathToFileURL(casePath))).default,observed=read(readPath),plain=read(plainPath);
assert(observed.finishedAt&&plain.finishedAt);assert.equal(observed.variant,'reads');assert.equal(plain.variant,'plain');
for(const report of[observed,plain]){
  for(const pin of report.inputs.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  assert(report.inputs.files.some(pin=>pin.path===casePath));
  const sealPin=report.inputs.files.find(pin=>pin.path!==casePath),seal=read(sealPin.path);
  for(const pin of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
  assert.equal(report.detectorFrozenBeforeChallenge,new Date(seal.frozenAt).getTime()<statSync(casePath).mtimeMs);
  assert(new Date(report.startedAt).getTime()>statSync(casePath).mtimeMs);
}
assert.deepEqual(observed.results.map(x=>x.id).sort(),cases.map(x=>x.id).sort());assert.deepEqual(plain.results.map(x=>x.id).sort(),cases.map(x=>x.id).sort());
const summary={stages:0,replayVariants:0,targets:0,targetHints:0,controls:0,quietControls:0,typingExclusions:0,observations:0,asyncObservations:0,primitiveReturnObservations:0,bodyOnlyReturnObservations:0,asyncInvocations:0,externalInvocationLinks:0,callbackObservations:0,asyncCallbackObservations:0,externalCallbackSlots:0,suppressions:0,asyncSuppressions:0,returnedDataSuppressions:0,retiredNonemptyBatches:0,automaticReloads:0,plainComparisons:0,misses:[],noisyControls:[],incorrectEarlierSuppressions:[]},audits=[];
const physical=new Map();
function disk(path){if(!physical.has(path))physical.set(path,ts.sys.readFile(path));return physical.get(path);}
function witness(frame,path,text,span){if(frame?.path!==path||frame.sourceSha256!==hash(text))return false;const source=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true),offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=span.start&&offset<span.end;}
for(const challenge of cases){
  const row=observed.results.find(x=>x.id===challenge.id),control=plain.results.find(x=>x.id===challenge.id);assert(!row.failure&&!control.failure);assert.deepEqual(row.pageErrors,[]);assert.deepEqual(control.pageErrors,[]);assert.deepEqual(row.stages.map(x=>x.id),challenge.stages.map(x=>x.id));
  let source=challenge.source,helper=challenge.stages[0].helper,config=JSON.stringify({compilerOptions:{target:'ESNext',module:'ESNext',moduleResolution:'bundler',jsx:'preserve',jsxImportSource:'@solidjs/web',strict:true,skipLibCheck:true,allowJs:true,...(challenge.noEmit?{noEmit:true}:{})},include:['src']});
  const root=join(dirname(readPath),challenge.id),path=join(root,'src/main.tsx'),helperPath=join(root,'src/consumer.ts'),configPath=join(root,'tsconfig.json');
  for(const[index,stage]of row.stages.entries()){
    await (async()=>{
    const plan=challenge.stages[index],same=control.stages[index];
    if(index){if(plan.mainComment)source=challenge.source+plan.mainComment;else if(plan.config)config=JSON.stringify(plan.config);else helper=plan.helper;}
    assert.equal(stage.sourceSha256,hash(source));assert.equal(stage.helperSha256,hash(helper));if(plan.afterUpdate===null)assert.equal(typeof stage.afterUpdate,'string');else assert.equal(stage.afterUpdate,plan.afterUpdate);assert.equal(stage.desired,plan.desired);
    for(const key of['sourceSha256','helperSha256','publishedTypingErrors','initial','afterUpdate','desired','behaviorPassed','feedback','errors',...(same.values===undefined?[]:['values']),...(stage.visibleText===undefined?[]:['visibleText'])])assert.deepEqual(stage[key],same[key],challenge.id+':'+stage.id+':'+key);
    assert.deepEqual(stage.errors,[]);assert.deepEqual(stage.current.notes.map(note=>note.severity),stage.current.notes.map(()=> 'info'));
    summary.stages++;summary.plainComparisons++;if(stage.automaticReload)summary.automaticReloads++;
    if(plan.typingCode){assert(stage.publishedTypingErrors.some(d=>d.code===plan.typingCode));assert.deepEqual(stage.current.notes,[]);assert.deepEqual(stage.current.suppressed,[]);summary.typingExclusions++;}
    else{assert.deepEqual(stage.publishedTypingErrors,[]);if(plan.role==='replay-only'){assert.equal(plan.initial,null);assert.equal(plan.afterUpdate,null);assert.equal(plan.desired,null);assert.equal(stage.behaviorPassed,null);summary.replayVariants++;}else if(plan.role==='target'){summary.targets++;assert(!stage.behaviorPassed);if(stage.current.notes.length)summary.targetHints++;else summary.misses.push(challenge.id+':'+stage.id);}else{summary.controls++;assert(stage.behaviorPassed);if(!stage.current.notes.length&&!stage.feedback.length)summary.quietControls++;else summary.noisyControls.push(challenge.id+':'+stage.id);}}
    const manifest=stage.inputManifest;assert.equal(hash(JSON.stringify(manifest)),stage.revision.inputSha256);
    const virtual=new Map([[path,source],[helperPath,helper],[configPath,config],...Object.entries(challenge.files??{}).map(([name,text])=>[join(root,'src',name),text])]);
    for(const input of manifest){
      if(input.kind==='read'){const text=virtual.has(input.path)?virtual.get(input.path):disk(input.path);assert.equal(text!==undefined,input.exists);assert.equal(text===undefined?null:hash(text),input.sha256,input.path);}
      else if(input.kind==='file')assert.equal(ts.sys.fileExists(input.path),input.exists);
      else if(input.kind==='directory')assert.equal(ts.sys.directoryExists(input.path),input.exists);
      else if(input.kind==='realpath')assert.equal(ts.sys.realpath?.(input.path)??input.path,input.result);
      else if(input.kind==='listing')assert.equal(JSON.stringify(ts.sys.readDirectory(...input.arguments.map(value=>value===null?undefined:value))),input.result);
      else if(input.kind==='directories')assert.equal(JSON.stringify(ts.sys.getDirectories(input.path)),input.result);
      else assert.fail('Unknown input kind');
    }
    assert(manifest.some(input=>input.kind==='read'&&input.path===path&&input.sha256===hash(source)));assert(manifest.some(input=>input.kind==='read'&&input.path===helperPath&&input.sha256===hash(helper)));
    const compiler=ts.convertCompilerOptionsFromJson(JSON.parse(config).compilerOptions,root).options,host=ts.createCompilerHost(compiler);
    host.getSourceFile=(name,language)=>{const text=virtual.has(name)?virtual.get(name):ts.sys.readFile(name);return text===undefined?undefined:ts.createSourceFile(name,text,language,true);};
    const publicProgram=ts.createProgram([...virtual.keys()].filter(name=>/\.[jt]sx?$/.test(name)),compiler,host);
    const diagnostics=ts.getPreEmitDiagnostics(publicProgram).filter(d=>d.category===ts.DiagnosticCategory.Error).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));assert.deepEqual(diagnostics,stage.publishedTypingErrors);
    for(const runtime of stage.runtimeSources??[]){assert.equal(hash(disk(runtime.path)),runtime.sourceSha256);assert.equal(hash(disk(runtime.metadata)),runtime.metadataSha256);const metadata=JSON.parse(disk(runtime.metadata));assert.equal(metadata.name,runtime.package);assert.equal(metadata.version,runtime.version);assert(runtime.path.startsWith(runtime.root+'/'));assert.equal(runtime.metadata,join(runtime.root,'package.json'));const syntax=ts.createSourceFile(runtime.path,disk(runtime.path),ts.ScriptTarget.Latest,true);assert(ts.isExternalModule(syntax));assert(manifest.some(input=>input.kind==='read'&&input.path===runtime.path&&input.sha256===runtime.sourceSha256));}
    const sourceOptions={...compiler,allowJs:true,checkJs:false,noEmit:true},sourceHost=ts.createCompilerHost(sourceOptions);sourceHost.getSourceFile=host.getSourceFile;
    const program=stage.runtimeSources?.length?ts.createProgram([...publicProgram.getRootFileNames(),...stage.runtimeSources.map(row=>row.path)],sourceOptions,sourceHost):publicProgram,checker=program.getTypeChecker();
    const sourceDiagnostics=stage.runtimeSources?.length?ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')})):[];assert.deepEqual(sourceDiagnostics,stage.sourceFactDiagnostics??[]);
    const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
    const exact=(source,start,end)=>{let found;function visit(node){if(node.getStart(source)===start&&node.end===end)found=node;ts.forEachChild(node,visit);}visit(source);return found;};
    for(const event of stage.events){
      assert.deepEqual(event.site.projectRevision,stage.revision);assert.equal(event.site.sourceSha256,hash(source));assert.equal(event.site.path,path);assert.deepEqual(event.context,{observer:false,owner:false});
      const transformed=row.instrumentation.transformed.find(item=>item.path===path&&JSON.stringify(item.session.revision)===JSON.stringify(stage.revision));assert(transformed);
      const {projectRevision,...site}=event.site;assert(transformed.sites.some(item=>JSON.stringify(item)===JSON.stringify(site)));
      if(event.callbackRegistration){const audited=auditCallbackSlot(program,event.callbackRegistration,event,program.getSourceFile(path),stage.revision,row.instrumentation.transformed);summary.callbackObservations++;if(event.asyncContinuation){summary.asyncCallbackObservations++;summary.asyncObservations++;summary.asyncInvocations+=event.asyncContinuation.chain.length;}if((stage.runtimeSources??[]).some(row=>row.path===audited.allocation))summary.externalCallbackSlots++;}
      else if(event.asyncContinuation){
        const chain=event.asyncContinuation.chain;assert(chain.length);const primitive=chain.every(link=>['null','undefined','boolean','number','string','bigint','symbol'].includes(link.returnedKind));assert.equal(event.asyncContinuation.completion,primitive?'explicit-primitive-normal-return':'explicit-normal-async-body-return');if(!primitive){assert.equal(event.asyncContinuation.promiseSettlement,'unobserved');assert.equal(event.asyncContinuation.resultFlow,'unproved');}else{assert.equal(event.asyncContinuation.promiseSettlement,undefined);assert.equal(event.asyncContinuation.resultFlow,undefined);}
        for(const link of chain){
          const meta=link.helper,bodyText=virtual.get(meta.function.path)??disk(meta.function.path),bodySource=program.getSourceFile(meta.function.path);assert(bodySource);
          assert.equal(meta.function.sha256,hash(bodyText));assert.deepEqual(meta.projectRevision,stage.revision);
          const fn=exact(bodySource,meta.function.start,meta.function.end);assert(fn&&ts.isFunctionLike(fn)&&fn.body&&!fn.asteriskToken&&fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword));
          assert(link.originalEntryFrames.some(frame=>witness(frame,meta.function.path,bodyText,meta.function)));
          assert(['null','undefined','boolean','number','string','bigint','symbol','object','function'].includes(link.returnedKind));assert(Number.isInteger(link.invocationId)&&link.invocationId>0);
          const ownReturns=[];function returns(node){if(node!==fn.body&&ts.isFunctionLike(node))return;if(ts.isReturnStatement(node))ownReturns.push(node);ts.forEachChild(node,returns);}returns(fn.body);if(!ts.isBlock(fn.body))ownReturns.push(fn.body);assert(ownReturns.length);
          const operation=exact(bodySource,link.operation.start,link.operation.end);assert(operation&&(ts.isCallExpression(operation)||ts.isPropertyAccessExpression(operation)));assert.equal(link.operation.sha256,hash(bodyText));
          let callee=ts.isCallExpression(operation)?operation.expression:operation;while(ts.isParenthesizedExpression(callee)||ts.isAsExpression(callee)||ts.isNonNullExpression(callee)||ts.isSatisfiesExpression(callee)||ts.isTypeAssertionExpression(callee))callee=callee.expression;
          const symbol=target(checker.getSymbolAtLocation(ts.isPropertyAccessExpression(callee)?callee.name:callee));assert(symbol?.declarations?.length);
          assert.deepEqual(link.operation.declarations,symbol.declarations.map(node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)})));
          assert(meta.operations.some(item=>JSON.stringify(item)===JSON.stringify(link.operation)));
          const served=row.instrumentation.transformed.find(item=>item.path===meta.function.path&&JSON.stringify(item.session.revision)===JSON.stringify(stage.revision));assert(served?.helpers.some(item=>JSON.stringify(item)===JSON.stringify((({projectRevision,...rest})=>rest)(meta))));summary.asyncInvocations++;if((stage.runtimeSources??[]).some(row=>row.path===meta.function.path))summary.externalInvocationLinks++;
        }
        const inner=chain[0],innerText=virtual.get(inner.helper.function.path)??disk(inner.helper.function.path);assert(event.nativeRead.originalFrames.some(frame=>witness(frame,inner.helper.function.path,innerText,inner.operation)));
        assert(chain.at(-1).originalEntryFrames.some(frame=>witness(frame,path,source,site)));summary.asyncObservations++;
      }else assert(event.originalFrames.some(frame=>witness(frame,path,source,site)));
      const reader=event.nativeRead.premise,readerText=disk(reader.path);assert.equal(hash(readerText),reader.sourceSha256);
      const span=event.identity.kind==='package-observer-guard'?reader.returned:reader.entry;
      assert(event.nativeRead.originalFrames.some(frame=>witness(frame,reader.path,readerText,span)));
      if(event.asyncContinuation){if(event.asyncContinuation.completion==='explicit-normal-async-body-return')summary.bodyOnlyReturnObservations++;else summary.primitiveReturnObservations++;}summary.observations++;
    }
    assert.equal(stage.current.acceptedEvents,stage.events.length);assert.deepEqual(stage.current.revision,stage.revision);
    for(const suppressed of stage.current.suppressed){if(suppressed.model.kind==='source-constant-own-data-body-return'){auditConstantReturnedData(program,suppressed.model,suppressed.observation);assert.equal(suppressed.scope,'returned-field-value-guidance-only');assert.equal(suppressed.effects,'open');assert.equal(suppressed.reactiveIntent,'open');assert.equal(suppressed.authority,false);assert.equal(suppressed.certification,false);assert(stage.current.readObservations.some(note=>JSON.stringify(note)===JSON.stringify(suppressed.observation)));summary.returnedDataSuppressions++;}else if(suppressed.model.kind==='source-async-primitive-constant-fulfillment'){auditAsyncConstant(program,program.getSourceFile(path),suppressed.model,suppressed.observation);summary.asyncSuppressions++;}else{assert.equal(suppressed.model.scope,'normal-completion-value-only');assert.equal(suppressed.model.authority,false);assert.equal(suppressed.model.certification,false);assert.equal(suppressed.model.function.path,helperPath);assert.equal(suppressed.model.function.sha256,hash(helper));assert.equal(suppressed.model.constant.value,'9');}summary.suppressions++;}
    for(const old of stage.retired){const prior=row.stages.find(s=>s.id===old.stage);assert(prior&&row.stages.indexOf(prior)<index);assert.equal(old.acceptedEvents,0);assert.deepEqual(old.notes,[]);assert.deepEqual(old.suppressed,[]);assert.equal(old.eventCount,prior.events.length);if(old.eventCount){assert.notDeepEqual(prior.revision,stage.revision);for(const event of prior.events)assert(old.open.some(item=>JSON.stringify(item.site)===JSON.stringify(event.site)&&/retired|issued/.test(item.reason)));summary.retiredNonemptyBatches++;}}
    if(plan.id==='constant-helper'&&stage.earlierProjectionOfInitialEvents?.suppressed.length)summary.incorrectEarlierSuppressions.push(challenge.id);
    audits.push({consumer:challenge.id,stage:stage.id,revision:stage.revision,recordedInputs:manifest.length,events:stage.events.length,oldBatches:stage.retired.filter(x=>x.eventCount).length});
    })();await new Promise(resolve=>setImmediate(resolve));globalThis.gc?.();audits.at(-1).collectedHeapBytes=process.memoryUsage().heapUsed;
  }
}
writeFileSync(out,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'independent input revisions, real typing programs, exact async helper/operation declarations, entry/read frames, explicit body-return provenance with separate primitive/body-only grades and unobserved Promise settlement across installed source and published typings, exact parameter-data-slot allocations and readonly member invocations with source callback entries, independently reconstructed constant fulfilled-value outcomes, constant own data-field initializers on normal async body return without a settlement claim, retired batches and plain behavior; native semantic models and runtime traces remain research premises',
  validators:['feedback-revision-audit-v15.mjs','callback-slot-audit-v4.mjs','constant-returned-data-audit-v1.mjs'].map(name=>{const path=new URL(name,import.meta.url).pathname;return{path,sha256:hash(readFileSync(path))};}),populationStatus:observed.populationStatus,inputs:[casePath,readPath,plainPath].map(path=>({path,sha256:hash(readFileSync(path))})),summary,audits},null,2)+'\n');console.log(JSON.stringify(summary,null,2));

