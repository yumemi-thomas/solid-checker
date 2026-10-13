// Independent app behavior, source-byte and typing-program audit; no detector imports.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {hash,read,closurePins} from './catalog.mjs';
import {ts} from './lower.mjs';
const [casesArg,readsArg,plainArg,outArg]=process.argv.slice(2),paths=[casesArg,readsArg,plainArg].map(value=>resolve(value)),out=resolve(outArg);
assert(!existsSync(out));const cases=(await import(pathToFileURL(paths[0]))).default,observed=read(paths[1]),plain=read(paths[2]);
assert(observed.finishedAt&&plain.finishedAt);assert.equal(observed.variant,'reads');assert.equal(plain.variant,'plain');
for(const report of [observed,plain]){
  assert.equal(report.authority,false);assert.equal(report.certification,false);
  for(const pin of report.inputs.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
  assert(report.inputs.files.some(pin=>pin.path===paths[0]));
  const pin=report.inputs.files.find(pin=>pin.path!==paths[0]),seal=read(pin.path);for(const item of [...seal.files,seal.baseline])assert.equal(hash(readFileSync(item.path)),item.sha256);
  for(const pkg of report.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
  for(const app of report.inputs.applications)for(const pin of app.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
}
assert.deepEqual(observed.results.map(row=>row.id),cases.map(row=>row.id));assert.deepEqual(plain.results.map(row=>row.id),cases.map(row=>row.id));
const rows=[];
for(const app of cases){
  await(async()=>{
    const row=observed.results.find(row=>row.id===app.id),control=plain.results.find(row=>row.id===app.id);assert(row.completed&&control.completed);assert(!row.failure&&!control.failure);
    assert.deepEqual(row.pageErrors,[]);assert.deepEqual(control.pageErrors,[]);assert.deepEqual(row.consoleErrors,control.consoleErrors);assert.deepEqual(row.blockedRequests,control.blockedRequests);assert.deepEqual(row.requestFailures,control.requestFailures);
    assert.deepEqual(row.offlineResponses,control.offlineResponses);for(const response of row.offlineResponses){const fixture=app.offlineResponses.find(item=>item.url===response.url);assert(fixture);assert.equal(response.bodySha256,hash(JSON.stringify(fixture.body)));}
    assert.deepEqual(row.steps.map(step=>step.id),app.actions.map(action=>action.id));assert.deepEqual(control.steps.map(step=>step.id),app.actions.map(action=>action.id));
    const loads=new Map();let transitions=0;
    for(const [index,step]of row.steps.entries()){
      const other=control.steps[index];for(const key of ['title','path','text','headings','links','modal','menuOpen','diagnostics'])assert.deepEqual(step.snapshot[key],other.snapshot[key],app.id+':'+step.id+':'+key);
      assert.equal(typeof step.snapshot.loadId,'string');assert.equal(typeof other.snapshot.loadId,'string');
      if(index){const reset=step.snapshot.loadId!==row.steps[index-1].snapshot.loadId;assert.equal(reset,other.snapshot.loadId!==control.steps[index-1].snapshot.loadId);if(reset)transitions++;}
      if(!loads.has(step.snapshot.loadId))loads.set(step.snapshot.loadId,{steps:0,stats:{}});const load=loads.get(step.snapshot.loadId);load.steps++;
      for(const [key,value]of Object.entries(step.snapshot.identityStats)){assert(Number.isSafeInteger(value)&&value>=0);assert(value>=(load.stats[key]??0),key+' decreased inside one page load');load.stats[key]=value;}
      assert.equal(step.snapshot.identityStats.eventRecords,step.snapshot.events.length);assert.equal(step.snapshot.identityStats.seenRecords,step.snapshot.events.length);
      assert.deepEqual(step.snapshot.events,[]);assert.deepEqual(step.projections,[]);assert.deepEqual(step.snapshot.continuationGaps,[]);
      assert.deepEqual(other.snapshot.events,[]);assert.deepEqual(other.projections,[]);
    }
    const manifest=row.inputManifest;assert.equal(hash(JSON.stringify(manifest)),row.revision.inputSha256);
    for(const input of manifest){
      if(input.kind==='read'){const text=ts.sys.readFile(input.path);assert.equal(text!==undefined,input.exists);assert.equal(text===undefined?null:hash(text),input.sha256,input.path);}
      else if(input.kind==='file')assert.equal(ts.sys.fileExists(input.path),input.exists);
      else if(input.kind==='directory')assert.equal(ts.sys.directoryExists(input.path),input.exists);
      else if(input.kind==='realpath')assert.equal(ts.sys.realpath?.(input.path)??input.path,input.result);
      else if(input.kind==='listing')assert.equal(JSON.stringify(ts.sys.readDirectory(...input.arguments.map(value=>value===null?undefined:value))),input.result);
      else if(input.kind==='directories')assert.equal(JSON.stringify(ts.sys.getDirectories(input.path)),input.result);
      else assert.fail('Unknown manifest input kind');
    }
    const configPath=join(app.root,'tsconfig.json'),config=ts.readConfigFile(configPath,ts.sys.readFile);assert(!config.error);
    const parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,app.root,{},configPath),publicProgram=ts.createProgram(parsed.fileNames,parsed.options);
    const errors=program=>ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).map(d=>({code:d.code,message:ts.flattenDiagnosticMessageText(d.messageText,'\n')}));
    assert.deepEqual(errors(publicProgram),row.publishedTypingErrors);assert.deepEqual(row.publishedTypingErrors,[]);
    for(const source of row.runtimeSources){assert.equal(hash(readFileSync(source.path)),source.sourceSha256);assert.equal(hash(readFileSync(source.metadata)),source.metadataSha256);const pkg=read(source.metadata);assert.equal(pkg.name,source.package);assert.equal(pkg.version,source.version);assert(source.path.startsWith(source.root+'/'));assert(manifest.some(input=>input.kind==='read'&&input.path===source.path&&input.sha256===source.sourceSha256));}
    const program=ts.createProgram([...publicProgram.getRootFileNames(),...row.runtimeSources.map(source=>source.path)],{...parsed.options,allowJs:true,checkJs:false,noEmit:true});assert.deepEqual(errors(program),row.sourceFactDiagnostics);
    const checker=program.getTypeChecker(),target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
    const exact=(source,start,end,predicate)=>{let found;function visit(node){if(node.getStart(source)===start&&node.end===end&&predicate(node))found=node;ts.forEachChild(node,visit);}visit(source);return found;};
    const sourceCache=new Map();const sourceOf=path=>{if(!sourceCache.has(path))sourceCache.set(path,program.getSourceFile(path));return sourceCache.get(path);};
    let sites=0,helpers=0;
    for(const served of row.instrumentation.transformed){assert.equal(hash(readFileSync(served.path)),served.sourceSha256);assert.deepEqual(served.session.revision,row.revision);const source=sourceOf(served.path);assert(source&&source.text===readFileSync(served.path,'utf8'));
      for(const site of served.sites){const node=exact(source,site.start,site.end,node=>ts.isCallExpression(node)||ts.isPropertyAccessExpression(node));assert(node);let callee=ts.isCallExpression(node)?node.expression:node;while(ts.isParenthesizedExpression(callee)||ts.isAsExpression(callee)||ts.isNonNullExpression(callee)||ts.isSatisfiesExpression(callee)||ts.isTypeAssertionExpression(callee))callee=callee.expression;const symbol=target(checker.getSymbolAtLocation(ts.isPropertyAccessExpression(callee)?callee.name:callee));assert(symbol?.declarations?.length);assert.deepEqual(site.calleeDeclarations,symbol.declarations.map(node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)})));sites++;}
      for(const helper of served.helpers){assert.equal(helper.function.path,served.path);assert.equal(helper.function.sha256,served.sourceSha256);assert(exact(source,helper.function.start,helper.function.end,node=>ts.isFunctionLike(node)&&!!node.body));helpers++;}
    }
    for(const premise of row.instrumentation.native)assert.equal(hash(readFileSync(premise.path)),premise.sourceSha256);
    assert.deepEqual(row.instrumentation.refused,[]);assert.deepEqual(row.instrumentation.nativeRefused,[]);assert.deepEqual(row.instrumentation.shortcutRefused,[]);
    const totals={};for(const load of loads.values())for(const [key,value]of Object.entries(load.stats))totals[key]=(totals[key]??0)+value;
    const repeats=row.steps.filter(step=>step.id.startsWith('contact-repeat-'));const retainedBuffers=repeats.map(step=>({eventRecords:step.snapshot.identityStats.eventRecords,seenRecords:step.snapshot.identityStats.seenRecords,metadataRecords:step.snapshot.identityStats.metadataRecords,gapRecords:step.snapshot.identityStats.gapRecords}));for(const buffer of retainedBuffers)assert.deepEqual(buffer,retainedBuffers[0]);
    rows.push({id:app.id,steps:row.steps.length,documentLoads:loads.size,documentTransitions:transitions,totals,sites,helpers,transformedModules:row.instrumentation.transformed.length,recordedInputs:manifest.length,runtimeModules:row.runtimeSources.length,offlineResponses:row.offlineResponses.length,events:0,hints:0,retainedBuffersDuringRepeatedContact:retainedBuffers[0],plainElapsedMs:control.elapsedMs,instrumentedElapsedMs:row.elapsedMs,firstPlainStepMs:control.steps[0].elapsedMs,firstInstrumentedStepMs:row.steps[0].elapsedMs,sessionStats:row.instrumentation.stats});
  })();await new Promise(resolve=>setImmediate(resolve));globalThis.gc?.();
}
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),scope:'independent unchanged retained app/source and public+implementation typing inputs, reviewed offline responses, visible/native-diagnostic behavior across matching page-load transitions, candidate execution/buffer counts and source declaration spans; no positive real-app defect, complete source discovery or precision-rate claim',validator:{path:new URL(import.meta.url).pathname,sha256:hash(readFileSync(new URL(import.meta.url)))},inputs:paths.map(path=>({path,sha256:hash(readFileSync(path))})),results:rows};writeFileSync(out,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(rows));
