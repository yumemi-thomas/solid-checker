// Independent audit: no selector, transform or hint projector is imported.
import assert from 'node:assert/strict';
import {existsSync,readFileSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {hash,read,packageRoot} from './catalog.mjs';
import {ts} from './lower.mjs';
import {oracleCompilerOptions} from '../../scripts/tsc-oracle.mjs';
const [populationArg,browserArg,studyArg,outArg]=process.argv.slice(2),inputs=[populationArg,browserArg,studyArg].map(path=>resolve(path)),output=resolve(outArg);
assert(!existsSync(output));const [population,browser,study]=inputs.map(read),witnesses=[];
for(const pin of [...population.files,...population.declarations,...study.inputs])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
for(const row of browser.results){
  const evaluated=study.results.find(item=>item.id===row.id);if(!evaluated.hints.length)continue;
  const path=join(dirname(inputs[1]),row.id,'src/main.tsx'),text=readFileSync(path,'utf8');
  const options=ts.convertCompilerOptionsFromJson({...oracleCompilerOptions('v2',true,{customConditions:['browser','development']}),allowJs:true},dirname(path)).options;
  const program=ts.createProgram([path],options),source=program.getSourceFile(path),checker=program.getTypeChecker();
  assert.equal(ts.getPreEmitDiagnostics(program).filter(d=>d.category===ts.DiagnosticCategory.Error).length,0);
  const calls=[],properties=[],functions=[];function visit(node){if(ts.isCallExpression(node))calls.push(node);if(ts.isPropertyAccessExpression(node))properties.push(node);if(ts.isFunctionLike(node))functions.push(node);ts.forEachChild(node,visit);}visit(source);
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  const unwrap=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
  const core=new Map(),namespaces=new Set();
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='solid-js'){
    if(statement.importClause?.namedBindings&&ts.isNamespaceImport(statement.importClause.namedBindings))namespaces.add(checker.getSymbolAtLocation(statement.importClause.namedBindings.name));
    for(const symbol of checker.getExportsOfModule(checker.getSymbolAtLocation(statement.moduleSpecifier)))if(symbol.getName()==='createMemo')core.set('createMemo',target(symbol));
  }
  function memoSymbol(expression,seen=new Set()){
    expression=unwrap(expression);if(ts.isPropertyAccessExpression(expression)){assert(ts.isIdentifier(unwrap(expression.expression))&&namespaces.has(checker.getSymbolAtLocation(unwrap(expression.expression))));return target(checker.getSymbolAtLocation(expression.name));}
    assert(ts.isIdentifier(expression));const symbol=checker.getSymbolAtLocation(expression);assert(symbol&&!seen.has(symbol));seen.add(symbol);const resolved=target(symbol);if(resolved===core.get('createMemo'))return resolved;
    assert.equal(symbol.declarations.length,1);const declaration=symbol.declarations[0];assert(ts.isVariableDeclaration(declaration)&&declaration.parent.flags&ts.NodeFlags.Const&&declaration.initializer);return memoSymbol(declaration.initializer,seen);
  }
  for(const note of evaluated.hints){
    assert.equal(note.severity,'info');assert.equal(note.category,'intent-open');assert.equal(note.staticDispatch,'open');assert.equal(note.certification,false);
    const site=note.witness,event=row.identityTrace.find(event=>JSON.stringify(event.site)===JSON.stringify(site)&&(note.guardShortcut?event.identity.kind==='package-observer-guard':event.identity.kind!=='package-observer-guard'));assert(event);
    assert.deepEqual(event.context,{owner:false,observer:false});const call=[...calls,...properties].find(node=>node.getStart(source)===site.start&&node.end===site.end);
    assert(call&&!call.questionDotToken&&!(call.flags&ts.NodeFlags.OptionalChain));
    const property=ts.isPropertyAccessExpression(call);if(!property)assert.equal(call.arguments.length,0);
    const expression=unwrap(property?call:call.expression),member=ts.isPropertyAccessExpression(expression);
    function simple(node){node=unwrap(node);return ts.isIdentifier(node)||ts.isPropertyAccessExpression(node)&&!node.questionDotToken&&simple(node.expression);}
    assert(ts.isIdentifier(expression)||member&&simple(expression.expression));
    const symbol=target(checker.getSymbolAtLocation(member?expression.name:expression));assert(symbol?.declarations?.length);
    assert.deepEqual(symbol.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),end:d.end,sha256:hash(d.getSourceFile().text)})),site.calleeDeclarations);
    const callback=functions.find(node=>node.getStart(source)===site.callback.start&&node.end===site.callback.end),memo=calls.find(node=>node.getStart(source)===site.memo.start&&node.end===site.memo.end);
    assert(callback&&memo);assert(memo.arguments[0]);assert.equal(memoSymbol(memo.expression),core.get('createMemo'));
    assert(call.getStart()>=callback.getStart()&&call.end<=callback.end);assert(callback.getStart()>memo.getStart()&&callback.end<memo.end);
    assert(site.memoDeclarations.length>0);for(const declaration of site.memoDeclarations)assert.equal(hash(readFileSync(declaration.path)),declaration.sha256);
    if(event.identity.kind==='package-observer-guard'){
      const model=event.identity.premise;assert.deepEqual(note.guardShortcut,model);assert.equal(model.valueFlow,'open');
      const packageText=readFileSync(model.path,'utf8');assert.equal(hash(packageText),model.sourceSha256);
      const packageOptions={allowJs:true,noEmit:true,noLib:true,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,customConditions:['browser','development']},packageProgram=ts.createProgram([model.path],packageOptions),packageSource=packageProgram.getSourceFile(model.path),packageChecker=packageProgram.getTypeChecker(),packageNodes=[];
      function packageVisit(node){packageNodes.push(node);ts.forEachChild(node,packageVisit);}packageVisit(packageSource);
      const find=span=>packageNodes.find(node=>node.getStart(packageSource)===span.start&&node.end===span.end),guard=find(model.guard),returned=find(model.returned),observer=find(model.observerCall),fn=find(model.function);
      assert(ts.isIfStatement(guard)&&ts.isReturnStatement(returned)&&ts.isCallExpression(observer)&&observer.arguments.length===0&&ts.isFunctionLike(fn));
      const condition=unwrap(guard.expression);assert(ts.isPrefixUnaryExpression(condition)&&condition.operator===ts.SyntaxKind.ExclamationToken&&unwrap(condition.operand)===observer);
      assert(guard.thenStatement===returned||ts.isBlock(guard.thenStatement)&&guard.thenStatement.statements.length===1&&guard.thenStatement.statements[0]===returned);
      assert(!fn.asteriskToken&&!fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword));
      function exactImport(call,name){
        assert(ts.isIdentifier(call.expression));const symbol=packageChecker.getSymbolAtLocation(call.expression);assert.equal(symbol.declarations.length,1);const declaration=symbol.declarations[0];assert(ts.isImportSpecifier(declaration));
        const statement=declaration.parent.parent.parent;assert(ts.isImportDeclaration(statement)&&['solid-js','@solidjs/signals'].includes(statement.moduleSpecifier.text));
        const exports=packageChecker.getExportsOfModule(packageChecker.getSymbolAtLocation(statement.moduleSpecifier)),expected=targetWith(packageChecker,exports.find(symbol=>symbol.getName()===name));assert.equal(targetWith(packageChecker,symbol),expected);return expected;
      }
      function targetWith(checker,symbol){return symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;}
      const declared=symbol=>symbol.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),end:d.end,sha256:hash(d.getSourceFile().text)}));
      assert.deepEqual(declared(exactImport(observer,'getObserver')),model.observerDeclarations);
      let parent=guard.parent;while(parent&&!ts.isFunctionLike(parent))parent=parent.parent;assert.equal(parent,fn);
      assert(model.signalPaths.length>0);for(const [index,span]of model.signalPaths.entries()){
        const factory=find(span);assert(ts.isCallExpression(factory)&&factory.getStart(packageSource)>returned.end);let parent=factory.parent;while(parent&&!ts.isFunctionLike(parent))parent=parent.parent;assert.equal(parent,fn);
        assert.deepEqual(declared(exactImport(factory,'createSignal')),model.signalDeclarations[index]);
      }
      assert.equal(model.accessorUses.length,model.signalPaths.length);
      const localFunction=node=>{let parent=node.parent;while(parent&&!ts.isFunctionLike(parent))parent=parent.parent;return parent;};
      function aliasBindings(node,base,seen=new Set()){
        node=unwrap(node);assert(ts.isIdentifier(node));const symbol=packageChecker.getSymbolAtLocation(node);assert(symbol&&!seen.has(symbol));if(symbol===base)return [];
        seen.add(symbol);assert.equal(symbol.declarations.length,1);const binding=symbol.declarations[0];assert(ts.isVariableDeclaration(binding)&&binding.parent.flags&ts.NodeFlags.Const&&binding.initializer&&localFunction(binding)===fn);
        return [...aliasBindings(binding.initializer,base,seen),{start:binding.getStart(packageSource),end:binding.end}];
      }
      for(const [index,facts]of model.accessorUses.entries()){
        assert(facts.length>0);const factory=find(model.signalPaths[index]);
        for(const fact of facts){assert.deepEqual(fact.factory,model.signalPaths[index]);assert.equal(fact.dispatch,'open');assert.equal(fact.resultFlow,'open');
          const use=find(fact.use),binding=fact.binding?find(fact.binding):null;assert(use&&localFunction(use)===fn);
          assert(fact.kind==='inline-index-call'?use.getStart(packageSource)<=factory.getStart(packageSource)&&use.end>factory.end:use.getStart(packageSource)>factory.end);
          let expression=factory;while(expression.parent&&unwrap(expression.parent)===factory)expression=expression.parent;
          if(fact.kind==='inline-index-call'){
            assert.equal(binding,null);const indexed=expression.parent;assert(ts.isElementAccessExpression(indexed)&&indexed.expression===expression&&ts.isNumericLiteral(indexed.argumentExpression)&&indexed.argumentExpression.text==='0');
            assert(ts.isCallExpression(use)&&use.expression===indexed&&use.arguments.length===0&&!use.questionDotToken);assert.deepEqual(fact.aliasBindings,[]);
          }else if(fact.kind==='tuple-index-call'){
            assert(ts.isVariableDeclaration(binding)&&ts.isIdentifier(binding.name)&&localFunction(binding)===fn);const base=packageChecker.getSymbolAtLocation(binding.name);let assigned=ts.isVariableDeclaration(expression.parent)&&expression.parent===binding&&expression.parent.initializer===expression;
            let current=expression;while(current.parent&&ts.isBinaryExpression(current.parent)&&current.parent.operatorToken.kind===ts.SyntaxKind.EqualsToken&&current.parent.right===current){if(ts.isIdentifier(unwrap(current.parent.left))&&packageChecker.getSymbolAtLocation(unwrap(current.parent.left))===base)assigned=true;current=current.parent;}assert(assigned);
            const indexed=unwrap(use.expression);assert(ts.isCallExpression(use)&&use.arguments.length===0&&!use.questionDotToken&&ts.isElementAccessExpression(indexed)&&!indexed.questionDotToken&&ts.isNumericLiteral(indexed.argumentExpression)&&indexed.argumentExpression.text==='0');assert.deepEqual(aliasBindings(indexed.expression,base),fact.aliasBindings);
            for(const write of packageNodes.filter(node=>localFunction(node)===fn&&node.getStart(packageSource)>factory.end&&node.getStart(packageSource)<use.getStart(packageSource)&&ts.isBinaryExpression(node)&&ts.isAssignmentOperator(node.operatorToken.kind))){
              const left=unwrap(write.left);assert(!ts.isIdentifier(left)||packageChecker.getSymbolAtLocation(left)!==base);
              if(ts.isElementAccessExpression(left)||ts.isPropertyAccessExpression(left)){let matches=false;try{aliasBindings(left.expression,base);matches=true;}catch{}assert(!matches);}
            }
          }else{
            assert(['accessor-call','accessor-object-escape'].includes(fact.kind)&&ts.isBindingElement(binding)&&ts.isIdentifier(binding.name)&&ts.isArrayBindingPattern(binding.parent)&&binding.parent.elements[0]===binding&&!binding.dotDotDotToken&&!binding.initializer);
            const declaration=binding.parent.parent;assert(ts.isVariableDeclaration(declaration)&&declaration.parent.flags&ts.NodeFlags.Const&&declaration.initializer===expression);const base=packageChecker.getSymbolAtLocation(binding.name);
            if(fact.kind==='accessor-call'){assert(ts.isCallExpression(use)&&use.arguments.length===0&&!use.questionDotToken);assert.deepEqual(aliasBindings(use.expression,base),fact.aliasBindings);}
            else if(ts.isShorthandPropertyAssignment(use)){assert.equal(packageChecker.getShorthandAssignmentValueSymbol(use),base);assert.deepEqual(fact.aliasBindings,[]);}
            else{assert(ts.isPropertyAssignment(use)&&(ts.isIdentifier(use.name)||ts.isStringLiteral(use.name)||ts.isNumericLiteral(use.name)));assert.deepEqual(aliasBindings(use.initializer,base),fact.aliasBindings);}
          }
        }
      }
      for(const pin of [...model.observerDeclarations,...model.ownerDeclarations,...model.signalDeclarations.flat()])assert.equal(hash(readFileSync(pin.path)),pin.sha256);
      const shared=join(packageRoot(dirname(model.path),'@solidjs/signals'),'dist/dev-shared.js');assert.equal(hash(readFileSync(shared)),'sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e');
      assert(event.nativeRead.originalFrames.some(frame=>{if(frame?.path!==model.path||frame.sourceSha256!==hash(packageText))return false;const offset=packageSource.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=returned.getStart()&&offset<returned.end;}));
      assert(event.originalFrames.some(frame=>{if(frame?.path!==path||frame.sourceSha256!==hash(text))return false;const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=call.getStart()&&offset<call.end;}));
      witnesses.push({id:row.id,channel:note.channel,read:{path,start:call.getStart(),end:call.end,sha256:hash(text)},packageShortcut:{path:model.path,start:returned.getStart(),end:returned.end,sha256:hash(packageText)},valueFlow:'open',occurrences:event.occurrences});continue;
    }
    // Inspect the exact native artifact independently, including its bound read.
    const premise=event.identity.premise,nativeText=readFileSync(premise.path,'utf8');
    assert.equal(hash(nativeText),'sha256:f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120');
    const native=ts.createSourceFile(premise.path,nativeText,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),nodes=[];
    function nativeVisit(node){nodes.push(node);ts.forEachChild(node,nativeVisit);}nativeVisit(native);
    const store=event.identity.kind==='native-store-target';assert(store||event.identity.kind==='native-node');
    const bySpan=span=>nodes.find(node=>node.getStart(native)===span.start&&node.end===span.end);
    const binding=bySpan(premise.binding),returned=bySpan(premise.returned);assert(ts.isVariableDeclaration(binding)&&ts.isIdentifier(returned));
    assert.equal(returned.text,binding.name.text);
    const reader=event.nativeRead.premise,readerText=readFileSync(reader.path,'utf8');let shared,readerFunction;
    if(store){
      assert.equal(reader.path,premise.path);assert.equal(typeof event.storeKey,'string');assert.notEqual(event.storeKey,'then');
      assert.deepEqual(reader,premise);shared=native;readerFunction=bySpan(reader.reader);
      const constructor=bySpan(premise.constructor),proxy=bySpan(premise.proxy),traps=bySpan(premise.traps),getter=bySpan(premise.getter),dispatch=bySpan(premise.dispatch);
      assert(ts.isFunctionDeclaration(constructor)&&constructor.body.statements.at(-1).expression===returned);
      assert(ts.isNewExpression(proxy)&&proxy.expression.text==='Proxy'&&proxy.arguments.length===2&&proxy.arguments[0].text===returned.text);
      assert(ts.isVariableDeclaration(traps)&&ts.isObjectLiteralExpression(traps.initializer)&&proxy.arguments[1].text===traps.name.text);
      assert(ts.isMethodDeclaration(getter)&&getter.name.text==='get'&&traps.initializer.properties.includes(getter));
      assert(getter.body.statements.at(-1)===dispatch&&ts.isCallExpression(dispatch.expression));
      assert.equal(dispatch.expression.expression.text,readerFunction.name.text);
      for(const index of [0,1])assert.equal(dispatch.expression.arguments[index].text,getter.parameters[index].name.text);
      assert.equal(readerFunction.parameters.length,6);
      // Rebind the constructor and trap arguments with a separate TypeScript program.
      const nativeOptions={allowJs:true,noResolve:true,noLib:true},host={...ts.createCompilerHost(nativeOptions),getSourceFile(file){return file===premise.path?native:undefined;}};
      const nativeProgram=ts.createProgram([premise.path],nativeOptions,host),nativeChecker=nativeProgram.getTypeChecker();
      const resolveNative=node=>nativeChecker.getSymbolAtLocation(node);
      assert.equal(resolveNative(returned),resolveNative(proxy.arguments[0]));assert.equal(resolveNative(returned).declarations[0],binding);
      assert.equal(resolveNative(proxy.arguments[1]).declarations[0],traps);
      assert.equal(resolveNative(dispatch.expression.expression).declarations[0],readerFunction);
      for(const index of [0,1])assert.equal(resolveNative(dispatch.expression.arguments[index]),resolveNative(getter.parameters[index].name));
    }else{
      const accessor=bySpan(premise.accessor);assert(ts.isFunctionDeclaration(accessor)&&accessor.parameters.length===1);
      assert(ts.isCallExpression(binding.initializer));assert.equal(binding.initializer.expression.name.text,'bind');
      assert.equal(binding.initializer.arguments[0].kind,ts.SyntaxKind.NullKeyword);assert.equal(binding.initializer.arguments[1].text,accessor.parameters[0].name.text);
      assert.equal(reader.path,join(dirname(premise.path),'dev-shared.js'));assert.equal(hash(readerText),'sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e');
      shared=ts.createSourceFile(reader.path,readerText,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS);
      readerFunction=shared.statements.find(node=>ts.isFunctionDeclaration(node)&&node.getStart(shared)===reader.reader.start&&node.end===reader.reader.end);
      assert(readerFunction&&readerFunction.parameters.length===1);assert.equal(readerFunction.parameters[0].name.text,reader.names.parameter);
      const importedReader=native.statements.flatMap(node=>ts.isImportDeclaration(node)&&node.moduleSpecifier.text==='./dev-shared.js'&&node.importClause?.namedBindings&&ts.isNamedImports(node.importClause.namedBindings)?node.importClause.namedBindings.elements:[])
        .find(specifier=>specifier.name.text===binding.initializer.expression.expression.text);assert.equal(importedReader.propertyName.text,'r');
      const exportedReader=shared.statements.flatMap(node=>ts.isExportDeclaration(node)&&node.exportClause&&ts.isNamedExports(node.exportClause)?node.exportClause.elements:[]).find(specifier=>specifier.name.text==='r');assert.equal(exportedReader.propertyName.text,readerFunction.name.text);
    }
    assert.equal(readerFunction.body.statements[0].getStart(),reader.entry.start);assert.equal(readerFunction.body.statements[0].end,reader.entry.end);
    assert(event.nativeRead.originalFrames.some(frame=>{if(frame?.path!==reader.path||frame.sourceSha256!==hash(readerText))return false;const offset=shared.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=reader.entry.start&&offset<reader.entry.end;}));
    assert(event.identity.originalCreationFrames.some(frame=>{
      if(frame?.path!==premise.path||frame.sourceSha256!==hash(nativeText))return false;
      const offset=native.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=returned.getStart()&&offset<returned.end;
    }));
    assert(event.originalFrames.some(frame=>{
      if(frame?.path!==path||frame.sourceSha256!==hash(text))return false;
      const offset=source.getPositionOfLineAndCharacter(frame.line-1,frame.column-1);return offset>=call.getStart()&&offset<call.end;
    }));
    witnesses.push({id:row.id,read:{path,start:call.getStart(),end:call.end,sha256:hash(text)},nativeCreation:{path:premise.path,start:returned.getStart(),end:returned.end,sha256:hash(nativeText)},occurrences:event.occurrences,nativeRead:{path:reader.path,start:reader.entry.start,end:reader.entry.end,sha256:hash(readerText)}});
  }
}
const report={authority:false,certification:false,finishedAt:new Date().toISOString(),inputs:inputs.map(path=>({path,sha256:hash(readFileSync(path))})),witnesses,
  limit:'Audit checks observed native creation and consumer provenance. Runtime trace integrity is research evidence, not an adversarial root of trust; callback intent stays open.'};
writeFileSync(output,JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({witnesses:witnesses.length}));

