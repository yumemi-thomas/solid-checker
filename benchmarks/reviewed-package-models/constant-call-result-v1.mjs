// Bounded source evidence for the value of an exact, stable function binding.
// This does not certify side effects, callback timing or dependency registration.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
export function primitiveConstant(node){
  if(!node)return {kind:'undefined',value:null};node=unwrapRead(node);
  if(ts.isNumericLiteral(node))return {kind:'number',value:String(Number(node.text))};
  if(ts.isPrefixUnaryExpression(node)&&[ts.SyntaxKind.PlusToken,ts.SyntaxKind.MinusToken].includes(node.operator)&&ts.isNumericLiteral(unwrapRead(node.operand))){
    const value=Number(unwrapRead(node.operand).text)*(node.operator===ts.SyntaxKind.MinusToken?-1:1);return {kind:'number',value:Object.is(value,-0)?'-0':String(value)};
  }
  if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return {kind:'string',value:node.text};
  if(node.kind===ts.SyntaxKind.TrueKeyword||node.kind===ts.SyntaxKind.FalseKeyword)return {kind:'boolean',value:node.kind===ts.SyntaxKind.TrueKeyword};
  if(node.kind===ts.SyntaxKind.NullKeyword)return {kind:'null',value:null};
  if(ts.isBigIntLiteral(node))return {kind:'bigint',value:String(BigInt(node.text.slice(0,-1)))};
  if(ts.isVoidExpression(node)&&ts.isNumericLiteral(unwrapRead(node.expression)))return {kind:'undefined',value:null};
  return null;
}
export function constantCallResult(program,source,call){
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
  const fn=resolveFunction(checker.getSymbolAtLocation(expression));if(!fn||fn.asteriskToken||fn.modifiers?.some(m=>m.kind===ts.SyntaxKind.AsyncKeyword))return null;
  const returns=[];
  if(!ts.isBlock(fn.body)){
    const constant=primitiveConstant(fn.body);if(!constant)return null;returns.push({span:span(fn.body),expression:span(fn.body),constant});
  }else{
    const statements=fn.body.statements.filter(statement=>!ts.isEmptyStatement(statement));
    if(!statements.length||!ts.isReturnStatement(statements.at(-1)))return null;
    let unknown=false;
    function visit(node){if(node!==fn.body&&ts.isFunctionLike(node))return;
      if(ts.isReturnStatement(node)){const constant=primitiveConstant(node.expression);if(!constant)unknown=true;else returns.push({span:span(node),expression:node.expression?span(node.expression):null,constant});}
      ts.forEachChild(node,visit);
    }visit(fn.body);if(unknown||!returns.length)return null;
  }
  if(returns.some(item=>JSON.stringify(item.constant)!==JSON.stringify(returns[0].constant)))return null;
  return {kind:'source-primitive-constant-return',function:span(fn),bindings,negativeInputs,returns,constant:returns[0].constant,
    dispatch:'exact-source-binding',scope:'normal-completion-value-only',authority:false,certification:false};
}
export function constantWholeCallbackResult(program,source,site){
  let call,returned;function visit(node){if(node.getStart(source)===site.start&&node.end===site.end)call=node;
    if(node.getStart(source)===site.returnedExpression.start&&node.end===site.returnedExpression.end)returned=node;ts.forEachChild(node,visit);}
  visit(source);if(!call||!returned||unwrapRead(returned)!==call||!ts.isCallExpression(call))return null;
  return constantCallResult(program,source,call);
}
