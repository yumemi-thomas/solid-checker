// Candidate calls need exact native memo facts; runtime identity proves reactivity.
// Runtime evidence is still needed; external callback/value flow remains open.
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
export function unwrapRead(node) {
  while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;
  return node;
}
export function nativeReadSites(program,source) {
  const checker=program.getTypeChecker(),core=new Map(),namespaces=new Set(),sites=[],open=[];
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='solid-js'){
    const module=checker.getSymbolAtLocation(statement.moduleSpecifier);
    if(statement.importClause?.namedBindings&&ts.isNamespaceImport(statement.importClause.namedBindings))namespaces.add(checker.getSymbolAtLocation(statement.importClause.namedBindings.name));
    for(const symbol of module?checker.getExportsOfModule(module):[])if(['createSignal','createMemo','untrack'].includes(symbol.getName()))core.set(symbol.getName(),target(symbol));
  }
  function coreSymbol(expression,seen=new Set()){
    expression=unwrapRead(expression);
    if(ts.isPropertyAccessExpression(expression)){
      const receiver=unwrapRead(expression.expression);
      if(!ts.isIdentifier(receiver)||!namespaces.has(checker.getSymbolAtLocation(receiver)))return null;
      return target(checker.getSymbolAtLocation(expression.name));
    }
    if(!ts.isIdentifier(expression))return null;
    const symbol=checker.getSymbolAtLocation(expression);if(!symbol||seen.has(symbol))return null;
    const resolved=target(symbol);if([...core.values()].includes(resolved))return resolved;
    const declaration=symbol.declarations?.length===1?symbol.declarations[0]:null;
    if(!declaration||!ts.isVariableDeclaration(declaration)||!(declaration.parent.flags&ts.NodeFlags.Const)||!declaration.initializer||declaration.getSourceFile()!==source)return null;
    return coreSymbol(declaration.initializer,new Set([...seen,symbol]));
  }
  function native(node,name){node=unwrapRead(node);return node&&ts.isCallExpression(node)&&core.has(name)&&coreSymbol(node.expression)===core.get(name);
  }
  function discardedCallback(callback){
    let expression=callback;
    while(expression.parent&&unwrapRead(expression.parent)===expression)expression=expression.parent;
    const call=expression.parent;
    if(!ts.isCallExpression(call)||!call.arguments.includes(expression)&&call.expression!==expression)return false;
    if(ts.isExpressionStatement(call.parent))return true;
    const declaration=call.parent;
    if(!ts.isVariableDeclaration(declaration)||!ts.isIdentifier(declaration.name)||!(declaration.parent.flags&ts.NodeFlags.Const))return false;
    const symbol=checker.getSymbolAtLocation(declaration.name);let references=0;
    function count(node){if(ts.isIdentifier(node)&&checker.getSymbolAtLocation(node)===symbol)references++;ts.forEachChild(node,count);}count(source);
    return references===1;
  }
  const functions=node=>{const result=[];for(let parent=node.parent;parent;parent=parent.parent)if(ts.isFunctionLike(parent))result.push(parent);return result;};
  function returned(node,callback){
    let current=node;
    while(current.parent&&current.parent!==callback){
      const parent=current.parent;
      if(ts.isReturnStatement(parent))return parent.expression===current?current:null;
      if(ts.isParenthesizedExpression(parent)||ts.isAsExpression(parent)||ts.isTypeAssertionExpression(parent)||ts.isNonNullExpression(parent)||ts.isSatisfiesExpression(parent)||ts.isAwaitExpression(parent))current=parent;
      else if(ts.isBinaryExpression(parent)&&![ts.SyntaxKind.CommaToken,ts.SyntaxKind.EqualsToken].includes(parent.operatorToken.kind)&&!ts.isAssignmentOperator(parent.operatorToken.kind))current=parent;
      else if(ts.isPropertyAccessExpression(parent)&&parent.expression===current&&!(parent.flags&ts.NodeFlags.OptionalChain))current=parent;
      else if(ts.isPropertyAssignment(parent)&&parent.initializer===current&&
        (ts.isIdentifier(parent.name)||ts.isStringLiteral(parent.name)||ts.isNumericLiteral(parent.name)))current=parent;
      else if(ts.isObjectLiteralExpression(parent)&&parent.properties.every(property=>
        ts.isPropertyAssignment(property)&&(ts.isIdentifier(property.name)||ts.isStringLiteral(property.name)||ts.isNumericLiteral(property.name))||
        ts.isShorthandPropertyAssignment(property)))current=parent;
      else if(ts.isArrayLiteralExpression(parent)&&!parent.elements.some(ts.isSpreadElement))current=parent;
      else if(ts.isConditionalExpression(parent)||ts.isTemplateSpan(parent)||ts.isTemplateExpression(parent)||ts.isPrefixUnaryExpression(parent)&&[ts.SyntaxKind.ExclamationToken,ts.SyntaxKind.PlusToken,ts.SyntaxKind.MinusToken].includes(parent.operator))current=parent;
      else return null;
    }
    return ts.isArrowFunction(callback)&&callback.body===current?current:null;
  }
  function simpleReceiver(node){node=unwrapRead(node);return ts.isIdentifier(node)||ts.isPropertyAccessExpression(node)&&!(node.flags&ts.NodeFlags.OptionalChain)&&simpleReceiver(node.expression);}
  function visit(node){
    const call=ts.isCallExpression(node)&&node.arguments.length===0&&!(node.flags&ts.NodeFlags.OptionalChain);
    const property=ts.isPropertyAccessExpression(node)&&!(node.flags&ts.NodeFlags.OptionalChain)&&simpleReceiver(node.expression)&&
      !(ts.isCallExpression(node.parent)&&node.parent.expression===node)&&!(ts.isPropertyAccessExpression(node.parent)&&node.parent.expression===node);
    if(call||property){
      const expression=unwrapRead(call?node.expression:node);
      const member=ts.isPropertyAccessExpression(expression)&&simpleReceiver(expression.expression)&&ts.isIdentifier(expression.name);
      const callee=ts.isIdentifier(expression)||member?target(checker.getSymbolAtLocation(member?expression.name:expression)):null;
      if(callee?.declarations?.length){
        const chain=functions(node),callback=chain[0],memo=chain.find(fn=>fn.parent&&native(fn.parent,'createMemo')&&fn.parent.arguments[0]===fn);
        if(memo&&callback!==memo&&(ts.isArrowFunction(callback)||ts.isFunctionExpression(callback)||ts.isFunctionDeclaration(callback))){
          const expression=returned(node,callback),intent=chain.some(fn=>fn.parent&&native(fn.parent,'untrack'));
          const discarded=discardedCallback(callback);
          if(!expression||intent||discarded)open.push({start:node.getStart(source),reason:discarded?'callback call result is discarded; reactive result flow stays open':intent?'exact native untrack expresses deliberate read intent':'read does not flow through an admitted returned expression'});
          else{
            const start=node.getStart(source),position=source.getLineAndCharacterOfPosition(start);
            sites.push({kind:property?'native-property-candidate':'native-read-candidate',path:source.fileName,sourceSha256:hash(source.text),start,end:node.end,line:position.line+1,column:position.character+1,
              calleeDeclarations:callee.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),end:d.end,sha256:hash(d.getSourceFile().text)})),
              memoDeclarations:core.get('createMemo').declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),sha256:hash(d.getSourceFile().text)})),
              callback:{start:callback.getStart(source),end:callback.end},memo:{start:memo.parent.getStart(source),end:memo.parent.end},
              returnedExpression:{start:expression.getStart(source),end:expression.end},staticDispatch:'open'});
          }
        }
      }
    }
    ts.forEachChild(node,visit);
  }
  visit(source);return {sites,open};
}
