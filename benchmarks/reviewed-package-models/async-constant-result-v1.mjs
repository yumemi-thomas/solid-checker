// Bounded normal fulfilled-value evidence, after an actual async-helper witness.
// Effects, rejection behavior, dependency registration and intent stay open.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
import {primitiveConstant} from './constant-call-result-v1.mjs';
export function asyncConstantCallResult(program,source,call){
  if(!ts.isCallExpression(call)||call.questionDotToken||call.flags&ts.NodeFlags.OptionalChain)return null;
  const checker=program.getTypeChecker(),target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  const span=node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)});
  let expression=unwrapRead(call.expression);
  if(ts.isPropertyAccessExpression(expression)){
    const receiver=unwrapRead(expression.expression),symbol=ts.isIdentifier(receiver)?checker.getSymbolAtLocation(receiver):null;
    if(expression.questionDotToken||!symbol||symbol.declarations?.length!==1||!ts.isNamespaceImport(symbol.declarations[0]))return null;
    expression=expression.name;
  }
  if(!ts.isIdentifier(expression))return null;
  const bindings=[],seen=new Set(),negativeInputs=[];
  function resolveFunction(symbol){
    symbol=target(symbol);if(!symbol||seen.has(symbol)||symbol.declarations?.length!==1)return null;seen.add(symbol);
    const declaration=symbol.declarations[0];if(declaration.getSourceFile().isDeclarationFile)return null;
    if(ts.isVariableDeclaration(declaration)){
      if(!(declaration.parent.flags&ts.NodeFlags.Const)||!ts.isIdentifier(declaration.name)||!declaration.initializer)return null;
      bindings.push({...span(declaration),kind:'const'});const initializer=unwrapRead(declaration.initializer);
      if(ts.isArrowFunction(initializer)||ts.isFunctionExpression(initializer))return initializer;
      return ts.isIdentifier(initializer)?resolveFunction(checker.getSymbolAtLocation(initializer)):null;
    }
    if(!ts.isFunctionDeclaration(declaration)||!declaration.body||!ts.isExternalModule(declaration.getSourceFile()))return null;
    let written=false,dynamic=false;
    function identifiers(node){
      if(ts.isIdentifier(node)&&target(checker.getSymbolAtLocation(node))===symbol)written=true;
      if(ts.isShorthandPropertyAssignment(node)&&target(checker.getShorthandAssignmentValueSymbol(node))===symbol)written=true;
      ts.forEachChild(node,identifiers);
    }
    function visit(node){
      if(ts.isBinaryExpression(node)&&ts.isAssignmentOperator(node.operatorToken.kind))identifiers(node.left);
      if((ts.isPrefixUnaryExpression(node)||ts.isPostfixUnaryExpression(node))&&[ts.SyntaxKind.PlusPlusToken,ts.SyntaxKind.MinusMinusToken].includes(node.operator))identifiers(node.operand);
      if(ts.isForOfStatement(node)||ts.isForInStatement(node))identifiers(node.initializer);
      if(node.getSourceFile()===declaration.getSourceFile()&&ts.isCallExpression(node)&&ts.isIdentifier(unwrapRead(node.expression))){
        const called=target(checker.getSymbolAtLocation(unwrapRead(node.expression)));
        if(called?.declarations?.some(d=>program.isSourceFileDefaultLibrary(d.getSourceFile())&&ts.isFunctionDeclaration(d)&&d.name?.text==='eval'))dynamic=true;
      }
      ts.forEachChild(node,visit);
    }
    for(const file of program.getSourceFiles())if(!file.isDeclarationFile){negativeInputs.push({path:file.fileName,sha256:hash(file.text)});visit(file);}
    if(written||dynamic)return null;bindings.push({...span(declaration),kind:'unwritten-function-declaration'});return declaration;
  }
  const fn=resolveFunction(checker.getSymbolAtLocation(expression));
  if(!fn?.body||fn.asteriskToken||!fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword))return null;
  let visits=0,exhausted=false;const certificate=[],returns=[];
  const normal={kind:'normal'},thrown={kind:'throw'},unknown={kind:'open'};
  function unique(items){const result=[...new Map(items.map(item=>[JSON.stringify(item),item])).values()];if(result.length>64){exhausted=true;return [unknown];}return result;}
  function constant(node){if(node&&ts.isAwaitExpression(unwrapRead(node)))node=unwrapRead(node).expression;return primitiveConstant(node);}
  function flow(node){
    if(++visits>256){exhausted=true;return [unknown];}let result;
    if(ts.isBlock(node)){
      result=[normal];for(const statement of node.statements){const next=flow(statement);result=unique(result.flatMap(item=>item.kind==='normal'?next:[item]));}
    }else if(ts.isReturnStatement(node)){
      const value=constant(node.expression);const returned={kind:'return',constant:value,origin:span(node)};returns.push({span:span(node),expression:node.expression?span(node.expression):null,constant:value});
      result=value?[returned]:[returned,thrown];
    }else if(ts.isThrowStatement(node))result=[thrown];
    else if(ts.isIfStatement(node))result=unique([...flow(node.thenStatement),...(node.elseStatement?flow(node.elseStatement):[normal]),thrown]);
    else if(ts.isTryStatement(node)){
      result=flow(node.tryBlock);
      if(node.catchClause){const caught=flow(node.catchClause.block);result=unique(result.flatMap(item=>item.kind==='throw'?caught:[item]));}
      if(node.finallyBlock){const final=flow(node.finallyBlock);result=unique(result.flatMap(item=>final.map(exit=>exit.kind==='normal'?item:exit)));}
    }else if(ts.isVariableStatement(node)||ts.isExpressionStatement(node)||ts.isClassDeclaration(node))result=[normal,thrown];
    else if(ts.isEmptyStatement(node)||ts.isDebuggerStatement(node)||ts.isFunctionDeclaration(node)||ts.isInterfaceDeclaration(node)||ts.isTypeAliasDeclaration(node))result=[normal];
    else result=[normal,thrown,unknown];
    certificate.push({node:span(node),syntaxKind:ts.SyntaxKind[node.kind],completions:result});return result;
  }
  const completions=ts.isBlock(fn.body)?flow(fn.body):[{kind:'return',constant:constant(fn.body),origin:span(fn.body)}];
  if(!ts.isBlock(fn.body))returns.push({span:span(fn.body),expression:span(fn.body),constant:constant(fn.body)});
  const fulfilled=completions.filter(item=>item.kind!=='throw').map(item=>item.kind==='normal'?{kind:'return',constant:{kind:'undefined',value:null}}:item);
  if(exhausted||!fulfilled.length||fulfilled.some(item=>item.kind!=='return'||!item.constant))return null;
  const value=fulfilled[0].constant;if(fulfilled.some(item=>JSON.stringify(item.constant)!==JSON.stringify(value)))return null;
  return {kind:'source-async-primitive-constant-fulfillment',function:span(fn),bindings,negativeInputs,returns,constant:value,certificate,completions,
    dispatch:'exact-source-binding',scope:'normal-fulfilled-value-only',bounds:{statements:256,completions:64},authority:false,certification:false};
}
export function asyncConstantWholeCallbackResult(program,source,site){
  let call,returned;function visit(node){if(node.getStart(source)===site.start&&node.end===site.end)call=node;
    if(node.getStart(source)===site.returnedExpression.start&&node.end===site.returnedExpression.end)returned=node;ts.forEachChild(node,visit);}
  visit(source);if(!call||!returned)return null;returned=unwrapRead(returned);if(ts.isAwaitExpression(returned))returned=unwrapRead(returned.expression);
  if(returned!==call||!ts.isCallExpression(call))return null;return asyncConstantCallResult(program,source,call);
}
