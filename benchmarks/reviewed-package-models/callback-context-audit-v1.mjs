// Independently resolve each runtime diagnostic's original native operation.
// This auditor imports neither the collector nor its deduplication code.
import assert from 'node:assert/strict';
import { existsSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { ts } from './lower.mjs';
import { hash, read, closurePins, packageRoot } from './catalog.mjs';
import { oracleCompilerOptions } from '../../scripts/tsc-oracle.mjs';
const [populationArg,browserArg,studyArg,outputArg]=process.argv.slice(2);
const populationPath=resolve(populationArg),browserPath=resolve(browserArg),studyPath=resolve(studyArg),output=resolve(outputArg);
const population=read(populationPath),browser=read(browserPath),study=read(studyPath);assert(!existsSync(output));
for(const pin of [...population.files,...population.declarations,...study.inputs,...study.evaluationInputs])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const profile=dirname(dirname(browserPath)),before=read(join(profile,'inputs-before.json')),after=read(join(profile,'inputs-after.json'));
assert.deepEqual(before.files,after.files);for(const pin of before.files)assert.equal(hash(readFileSync(pin.path)),pin.sha256);
for(const pkg of population.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);
const witnesses=[],occurrenceChecks=[],quiet=[],excluded=[];
for(const row of browser.results){
  const expected=population.rows.find(item=>item.id===row.id),evaluated=study.results.find(item=>item.id===row.id);
  const root=join(dirname(browserPath),row.id),path=join(root,'src/main.tsx'),text=readFileSync(path,'utf8');
  assert.equal(hash(text),expected.sourceSha256);assert.equal(row.sourceSha256,expected.sourceSha256);
  const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},root).options;
  const program=ts.createProgram([path],options),source=program.getSourceFile(path),checker=program.getTypeChecker();
  const errors=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.deepEqual(errors.map(d=>d.code).sort(),row.publishedTypingErrors.map(d=>d.code).sort());
  if(errors.length){assert(row.excludedBeforeExecution);assert.equal(evaluated.feedback.length,0);excluded.push(row.id);continue;}
  const native=new Map(),calls=[];
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='solid-js'){
    const module=checker.getSymbolAtLocation(statement.moduleSpecifier);assert(module);
    for(const exported of checker.getExportsOfModule(module))if(['createSignal','onCleanup','runWithOwner'].includes(exported.getName()))native.set(exported.getName(),target(exported));
  }
  function visit(node){if(ts.isCallExpression(node))calls.push(node);ts.forEachChild(node,visit);}visit(source);
  function coreCall(call,name){return ts.isIdentifier(call.expression)&&target(checker.getSymbolAtLocation(call.expression))===native.get(name);}
  function setter(call){
    if(!ts.isIdentifier(call.expression))return false;
    const symbol=target(checker.getSymbolAtLocation(call.expression));if(symbol?.declarations?.length!==1)return false;
    const binding=symbol.declarations[0];if(!ts.isBindingElement(binding)||!ts.isArrayBindingPattern(binding.parent)||binding.parent.elements.indexOf(binding)!==1)return false;
    const declaration=binding.parent.parent;
    return ts.isVariableDeclaration(declaration)&&ts.isCallExpression(declaration.initializer)&&coreCall(declaration.initializer,'createSignal');
  }
  const notes=row.feedback.filter(note=>note.category==='execution');
  for(const note of notes){
    assert.equal(note.certification,false);assert.equal(note.basis,'runtime-observation');
    assert(note.originalLocation?.path===path);assert.equal(note.consumerFrames.length,note.frames.filter(frame=>new URL(frame.path).pathname.startsWith('/src/')).length);
    const position=source.getPositionOfLineAndCharacter(note.originalLocation.line-1,note.originalLocation.column-1);
    const relevant=calls.filter(call=>call.expression.getStart(source)<=position&&position<call.expression.end&&
      (note.code==='REACTIVE_WRITE_IN_OWNED_SCOPE'?setter(call):coreCall(call,note.code==='RUN_WITH_DISPOSED_OWNER'?'runWithOwner':'onCleanup')));
    assert.equal(relevant.length,1,JSON.stringify({id:row.id,code:note.code,location:note.originalLocation}));
    const call=relevant[0],symbol=target(checker.getSymbolAtLocation(call.expression));
    const appFrames=note.frames.flatMap((frame,index)=>new URL(frame.path).pathname.startsWith('/src/')?[note.originalFrames[index]]:[]);
    assert(appFrames.length&&appFrames.every(frame=>frame?.path===path));
    assert(Number.isInteger(note.occurrences)&&note.occurrences>0);
    witnesses.push({id:row.id,code:note.code,operation:{start:call.getStart(source),end:call.end,text:call.getText(source)},
      sourceSha256:hash(text),declaration:symbol.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),sha256:hash(d.getSourceFile().text)})),
      consumerPath:appFrames,occurrences:note.occurrences,authority:false,certification:false});
  }
  const expectedOccurrences=expected.provenance.expectedOccurrences;
  if(expectedOccurrences){const actual=notes.filter(note=>expected.provenance.codes.includes(note.code)).map(note=>note.occurrences).sort();
    assert.deepEqual(actual,[...expectedOccurrences].sort(),row.id);occurrenceChecks.push({id:row.id,actual});}
  const codes=[...new Set(notes.map(note=>note.code))];
  for(const code of codes)assert.equal(notes.filter(note=>note.code===code).reduce((n,note)=>n+note.occurrences,0),row.observations.filter(note=>note.channelCode===code).length);
  if(!evaluated.feedback.length&&!row.harnessFailure)quiet.push(row.id);
}
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),
  inputs:[populationPath,browserPath,studyPath,new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))})),
  checks:['original source and published typings','exact native call or exact native signal setter binding','mapped consumer frames','complete observed diagnostic occurrence count','independent expected occurrence checks'],
  diagnosticWitnesses:witnesses.length,witnesses,occurrenceChecks,quiet,excluded};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({witnesses:witnesses.length,occurrenceChecks:occurrenceChecks.length,quiet:quiet.length,excluded:excluded.length}));
