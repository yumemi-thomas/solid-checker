// Independent AST/type audit of observed reads and informational hint spans.
// Neither the site selector, transform nor feedback projector is imported.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {resolve,dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {hash,read} from './catalog.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';
const [populationArg,browserArg,studyArg,outputArg]=process.argv.slice(2),populationPath=resolve(populationArg),browserPath=resolve(browserArg),studyPath=resolve(studyArg),output=resolve(outputArg);
const population=read(populationPath),browser=read(browserPath),study=read(studyPath);assert(!existsSync(output));
for(const pin of [...population.files,...population.declarations,...study.inputs])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
const witnesses=[],excluded=[];
for(const row of browser.results){
  const selected=population.rows.find(item=>item.id===row.id),evaluated=study.results.find(item=>item.id===row.id),path=join(dirname(browserPath),row.id,'src/main.tsx'),text=readFileSync(path,'utf8');
  assert.equal(hash(text),selected.sourceSha256);assert.equal(row.sourceSha256,selected.sourceSha256);
  const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},dirname(path)).options;
  const program=ts.createProgram([path],options),source=program.getSourceFile(path),checker=program.getTypeChecker(),errors=ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error);
  assert.deepEqual(errors.map(d=>d.code).sort(),row.publishedTypingErrors.map(d=>d.code).sort());
  if(errors.length){assert(row.excludedBeforeExecution);assert.equal(evaluated.feedback.length,0);excluded.push(row.id);continue;}
  const functions=[],calls=[],core=new Map(),namespaces=new Set();
  const unwrap=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='solid-js'){
    if(statement.importClause?.namedBindings&&ts.isNamespaceImport(statement.importClause.namedBindings))namespaces.add(checker.getSymbolAtLocation(statement.importClause.namedBindings.name));
    const module=checker.getSymbolAtLocation(statement.moduleSpecifier);for(const exported of checker.getExportsOfModule(module))if(['createSignal','createMemo'].includes(exported.getName()))core.set(exported.getName(),target(exported));
  }
  function scan(node){if(ts.isCallExpression(node))calls.push(node);if(ts.isFunctionLike(node))functions.push(node);ts.forEachChild(node,scan);}scan(source);
  function nativeCallee(node,seen=new Set()){
    node=unwrap(node);if(ts.isPropertyAccessExpression(node)){const receiver=unwrap(node.expression);assert(ts.isIdentifier(receiver)&&namespaces.has(checker.getSymbolAtLocation(receiver)));return target(checker.getSymbolAtLocation(node.name));}
    assert(ts.isIdentifier(node));const symbol=checker.getSymbolAtLocation(node);assert(symbol&&!seen.has(symbol));seen.add(symbol);const resolved=target(symbol);
    if([...core.values()].includes(resolved))return resolved;
    assert.equal(symbol.declarations.length,1);const declaration=symbol.declarations[0];assert(ts.isVariableDeclaration(declaration)&&declaration.parent.flags&ts.NodeFlags.Const&&declaration.initializer);
    return nativeCallee(declaration.initializer,seen);
  }
  function accessor(node,aliases=[],seen=new Set()){
    const symbol=checker.getSymbolAtLocation(unwrap(node.expression));assert(symbol&&!seen.has(symbol));seen.add(symbol);assert.equal(symbol.declarations.length,1);
    const declaration=symbol.declarations[0];let binding=declaration;
    if(ts.isBindingElement(declaration)){assert(ts.isArrayBindingPattern(declaration.parent));assert.equal(declaration.parent.elements.indexOf(declaration),0);binding=declaration.parent.parent;}
    assert(ts.isVariableDeclaration(binding)&&binding.parent.flags&ts.NodeFlags.Const&&binding.initializer);
    const init=unwrap(binding.initializer);
    if(ts.isIdentifier(init)){aliases.unshift({start:binding.getStart(source),end:binding.end});return accessor({expression:init},aliases,seen);}
    assert(ts.isCallExpression(init));const kind=nativeCallee(init.expression);assert(kind===core.get('createSignal')||kind===core.get('createMemo'));
    return {binding,creation:init,aliases,kind};
  }
  for(const note of evaluated.feedback.filter(note=>note.channel==='observed-reactive-read')){
    assert.equal(note.code,'OBSERVED_MEMO_CALLBACK_UNTRACKED_READ');assert.equal(note.severity,'info');assert.equal(note.category,'intent-open');assert.equal(note.staticDispatch,'open');assert.equal(note.certification,false);
    const site=note.witness,event=row.readTrace.find(event=>JSON.stringify(event.site)===JSON.stringify(site));assert(event);assert.deepEqual(event.context,{observer:false,owner:false});assert.equal(site.sourceSha256,hash(text));
    const readCall=calls.find(call=>call.getStart(source)===site.start&&call.end===site.end);assert(readCall&&readCall.arguments.length===0);
    const resolved=accessor(readCall);assert.deepEqual(site.accessorBinding,{start:resolved.binding.getStart(source),end:resolved.binding.end});assert.deepEqual(site.accessorAliases??[],resolved.aliases);
    assert.deepEqual(site.creation,{start:resolved.creation.getStart(source),end:resolved.creation.end,declarations:resolved.kind.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),sha256:hash(d.getSourceFile().text)}))});
    const callback=functions.find(fn=>fn.getStart(source)===site.callback.start&&fn.end===site.callback.end);assert(callback&&(ts.isArrowFunction(callback)||ts.isFunctionExpression(callback)||ts.isFunctionDeclaration(callback)));
    const memo=calls.find(call=>call.getStart(source)===site.memo.start&&call.end===site.memo.end);assert(memo&&nativeCallee(memo.expression)===core.get('createMemo'));
    let ancestor=readCall,insideCallback=false,insideMemo=false;
    while(ancestor){if(ancestor===callback)insideCallback=true;if(ancestor===memo)insideMemo=true;ancestor=ancestor.parent;}assert(insideCallback&&insideMemo);
    assert.equal(note.location.path,path);assert.equal(note.location.startByte,Buffer.byteLength(text.slice(0,site.start)));
    assert(event.originalFrames.some(frame=>frame?.path===path&&frame.sourceSha256===hash(text)));
    const position=source.getPositionOfLineAndCharacter(event.originalLocation.line-1,event.originalLocation.column-1);
    assert(position>=callback.getStart(source)&&position<callback.end);
    witnesses.push({id:row.id,start:site.start,end:site.end,sourceSha256:hash(text),context:event.context,occurrences:event.occurrences,
      accessorCreation:site.creation,aliases:site.accessorAliases??[],memo:site.memo,callback:site.callback,certification:false});
  }
}
writeFileSync(output,JSON.stringify({authority:false,certification:false,finishedAt:new Date().toISOString(),
  inputs:[populationPath,browserPath,studyPath,new URL(import.meta.url).pathname].map(path=>({path,sha256:hash(readFileSync(path))})),
  checks:['published typing boundary','exact native creation and immutable aliases','current read and callback/memo source spans','native absent-context observation','mapped source and informational severity'],
  witnesses,excluded},null,2)+'\n');console.log(JSON.stringify({witnesses:witnesses.length,excluded:excluded.length}));
