// Independent AST outcome-set check. No result-proof, selector or projector import.
import assert from 'node:assert/strict';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
export function auditAsyncConstant(program,source,model,observation){
  assert.equal(model.kind,'source-async-primitive-constant-fulfillment');assert.equal(model.scope,'normal-fulfilled-value-only');assert.equal(model.authority,false);assert.equal(model.certification,false);
  const checker=program.getTypeChecker(),target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  const span=node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)});
  const strip=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
  function exact(file,start,end){let found;function visit(node){if(node.getStart(file)===start&&node.end===end)found=node;ts.forEachChild(node,visit);}visit(file);return found;}
  const call=exact(source,observation.witness.start,observation.witness.end);assert(call&&ts.isCallExpression(call));
  let returned=strip(exact(source,observation.witness.returnedExpression.start,observation.witness.returnedExpression.end));if(returned&&ts.isAwaitExpression(returned))returned=strip(returned.expression);assert.equal(returned,call);
  let callee=strip(call.expression);assert(!call.questionDotToken&&!(call.flags&ts.NodeFlags.OptionalChain));
  if(ts.isPropertyAccessExpression(callee)){const receiver=strip(callee.expression);assert(ts.isIdentifier(receiver));const symbol=checker.getSymbolAtLocation(receiver);assert(symbol?.declarations?.length===1&&ts.isNamespaceImport(symbol.declarations[0]));callee=callee.name;}
  assert(ts.isIdentifier(callee));let symbol=target(checker.getSymbolAtLocation(callee)),fn;const expectedBindings=[],seen=new Set();
  while(symbol){assert(!seen.has(symbol));seen.add(symbol);assert.equal(symbol.declarations?.length,1);const declaration=symbol.declarations[0];assert(!declaration.getSourceFile().isDeclarationFile);
    if(ts.isVariableDeclaration(declaration)){assert(declaration.parent.flags&ts.NodeFlags.Const);assert(ts.isIdentifier(declaration.name)&&declaration.initializer);expectedBindings.push({...span(declaration),kind:'const'});const value=strip(declaration.initializer);if(ts.isArrowFunction(value)||ts.isFunctionExpression(value)){fn=value;break;}assert(ts.isIdentifier(value));symbol=target(checker.getSymbolAtLocation(value));continue;}
    assert(ts.isFunctionDeclaration(declaration)&&declaration.body&&ts.isExternalModule(declaration.getSourceFile()));expectedBindings.push({...span(declaration),kind:'unwritten-function-declaration'});fn=declaration;
    function written(node){if(ts.isIdentifier(node))assert.notEqual(target(checker.getSymbolAtLocation(node)),symbol);if(ts.isShorthandPropertyAssignment(node))assert.notEqual(target(checker.getShorthandAssignmentValueSymbol(node)),symbol);ts.forEachChild(node,written);}
    function check(node){if(ts.isBinaryExpression(node)&&ts.isAssignmentOperator(node.operatorToken.kind))written(node.left);if((ts.isPrefixUnaryExpression(node)||ts.isPostfixUnaryExpression(node))&&[ts.SyntaxKind.PlusPlusToken,ts.SyntaxKind.MinusMinusToken].includes(node.operator))written(node.operand);if(ts.isForOfStatement(node)||ts.isForInStatement(node))written(node.initializer);
      if(node.getSourceFile()===fn.getSourceFile()&&ts.isCallExpression(node)&&ts.isIdentifier(strip(node.expression))){const called=target(checker.getSymbolAtLocation(strip(node.expression)));assert(!called?.declarations?.some(d=>program.isSourceFileDefaultLibrary(d.getSourceFile())&&ts.isFunctionDeclaration(d)&&d.name?.text==='eval'));}ts.forEachChild(node,check);}
    const files=program.getSourceFiles().filter(file=>!file.isDeclarationFile);assert.deepEqual(model.negativeInputs,files.map(file=>({path:file.fileName,sha256:hash(file.text)})));for(const file of files)check(file);break;
  }
  assert(fn?.body&&!fn.asteriskToken&&fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword));assert.deepEqual(span(fn),model.function);assert.deepEqual(expectedBindings,model.bindings);
  assert(observation.asyncContinuation?.chain.some(link=>JSON.stringify(link.helper.function)===JSON.stringify(model.function)));
  function literal(node){node=strip(node);if(!node)return {kind:'undefined',value:null};if(ts.isAwaitExpression(node))return literal(node.expression);
    if(ts.isNumericLiteral(node))return {kind:'number',value:String(Number(node.text))};if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return {kind:'string',value:node.text};
    if(node.kind===ts.SyntaxKind.NullKeyword)return {kind:'null',value:null};if(node.kind===ts.SyntaxKind.TrueKeyword||node.kind===ts.SyntaxKind.FalseKeyword)return {kind:'boolean',value:node.kind===ts.SyntaxKind.TrueKeyword};
    if(ts.isBigIntLiteral(node))return {kind:'bigint',value:BigInt(node.text.slice(0,-1)).toString()};
    if(ts.isVoidExpression(node)&&ts.isNumericLiteral(strip(node.expression)))return {kind:'undefined',value:null};
    if(ts.isPrefixUnaryExpression(node)&&[ts.SyntaxKind.PlusToken,ts.SyntaxKind.MinusToken].includes(node.operator)&&ts.isNumericLiteral(strip(node.operand))){const magnitude=Number(strip(node.operand).text),number=node.operator===ts.SyntaxKind.MinusToken?-magnitude:magnitude;return {kind:'number',value:Object.is(number,-0)?'-0':String(number)};}return null;
  }
  const outcome=(falls,throws,values=[])=>({falls,throws,values:new Map(values.map(value=>[JSON.stringify(value),value]))});
  const combine=(a,b)=>outcome(a.falls||b.falls,a.throws||b.throws,[...a.values.values(),...b.values.values()]);let nodes=0;
  function paths(node){assert(++nodes<=512,'independent outcome check exceeded its budget');
    if(ts.isBlock(node)){let result=outcome(true,false);for(const statement of node.statements){const next=paths(statement);if(result.falls)result=outcome(next.falls,result.throws||next.throws,[...result.values.values(),...next.values.values()]);}return result;}
    if(ts.isReturnStatement(node)){const value=literal(node.expression);return outcome(false,!value,[value]);}
    if(ts.isThrowStatement(node))return outcome(false,true);
    if(ts.isIfStatement(node)){const a=paths(node.thenStatement),b=node.elseStatement?paths(node.elseStatement):outcome(true,false);const merged=combine(a,b);merged.throws=true;return merged;}
    if(ts.isTryStatement(node)){let result=paths(node.tryBlock);if(node.catchClause){const caught=paths(node.catchClause.block);if(result.throws)result=outcome(result.falls||caught.falls,caught.throws,[...result.values.values(),...caught.values.values()]);}
      if(node.finallyBlock){const final=paths(node.finallyBlock);result=outcome(result.falls&&final.falls,final.throws||result.throws&&final.falls,[...final.values.values(),...(final.falls?result.values.values():[])]);}return result;}
    if(ts.isVariableStatement(node)||ts.isExpressionStatement(node)||ts.isClassDeclaration(node))return outcome(true,true);
    if(ts.isEmptyStatement(node)||ts.isDebuggerStatement(node)||ts.isFunctionDeclaration(node)||ts.isInterfaceDeclaration(node)||ts.isTypeAliasDeclaration(node))return outcome(true,false);
    return outcome(true,true,[null]);
  }
  const result=ts.isBlock(fn.body)?paths(fn.body):outcome(false,!literal(fn.body),[literal(fn.body)]);if(result.falls)result.values.set('implicit-undefined',{kind:'undefined',value:null});assert(result.values.size>0);for(const value of result.values.values())assert.deepEqual(value,model.constant);
  for(const item of model.certificate){assert.equal(item.node.path,fn.getSourceFile().fileName);assert.equal(item.node.sha256,hash(fn.getSourceFile().text));const node=exact(fn.getSourceFile(),item.node.start,item.node.end);assert(node);assert.equal(ts.SyntaxKind[node.kind],item.syntaxKind);}
  return {function:model.function,constant:model.constant,independentOutcomeNodes:nodes,fulfilledAlternatives:result.values.size};
}
