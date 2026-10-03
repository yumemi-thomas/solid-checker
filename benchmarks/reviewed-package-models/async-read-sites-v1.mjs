// Exact native accessor reads returned by callbacks lexically inside a memo.
// Runtime evidence is still needed; external callback/value flow remains open.
import { hash } from './catalog.mjs';
import { ts } from './lower.mjs';
export function unwrapRead(node) {
  while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;
  return node;
}
export function asyncReadSites(program,source) {
  const checker=program.getTypeChecker(),core=new Map(),bindings=new Map(),sites=[],open=[];
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text==='solid-js'){
    const module=checker.getSymbolAtLocation(statement.moduleSpecifier);
    for(const symbol of module?checker.getExportsOfModule(module):[])if(['createSignal','createMemo','untrack'].includes(symbol.getName()))core.set(symbol.getName(),target(symbol));
  }
  function native(node,name){node=unwrapRead(node);if(!node||!ts.isCallExpression(node))return false;
    const callee=unwrapRead(node.expression),symbol=checker.getSymbolAtLocation(ts.isPropertyAccessExpression(callee)?callee.name:callee);
    return core.has(name)&&target(symbol)===core.get(name);
  }
  function constants(node){
    if(ts.isVariableDeclaration(node)&&node.initializer&&node.parent.flags&ts.NodeFlags.Const){
      const init=unwrapRead(node.initializer);let name=null;
      if(native(init,'createSignal')&&ts.isArrayBindingPattern(node.name)&&ts.isBindingElement(node.name.elements[0]))name=node.name.elements[0].name;
      if(native(init,'createMemo')&&ts.isIdentifier(node.name))name=node.name;
      if(name&&ts.isIdentifier(name)){const symbol=checker.getSymbolAtLocation(name);if(symbol?.declarations?.length===1)bindings.set(symbol,{binding:node,creation:init});}
    }
    ts.forEachChild(node,constants);
  }
  constants(source);
  const functions=node=>{const result=[];for(let parent=node.parent;parent;parent=parent.parent)if(ts.isFunctionLike(parent))result.push(parent);return result;};
  function returned(node,callback){
    let current=node;
    while(current.parent&&current.parent!==callback){
      const parent=current.parent;
      if(ts.isReturnStatement(parent))return parent.expression===current?current:null;
      if(ts.isParenthesizedExpression(parent)||ts.isAsExpression(parent)||ts.isTypeAssertionExpression(parent)||ts.isNonNullExpression(parent)||ts.isSatisfiesExpression(parent)||ts.isAwaitExpression(parent))current=parent;
      else if(ts.isBinaryExpression(parent)&&![ts.SyntaxKind.CommaToken,ts.SyntaxKind.EqualsToken].includes(parent.operatorToken.kind)&&!ts.isAssignmentOperator(parent.operatorToken.kind))current=parent;
      else if(ts.isConditionalExpression(parent)||ts.isTemplateSpan(parent)||ts.isTemplateExpression(parent)||ts.isPrefixUnaryExpression(parent)&&[ts.SyntaxKind.ExclamationToken,ts.SyntaxKind.PlusToken,ts.SyntaxKind.MinusToken].includes(parent.operator))current=parent;
      else return null;
    }
    return ts.isArrowFunction(callback)&&callback.body===current?current:null;
  }
  function visit(node){
    if(ts.isCallExpression(node)&&node.arguments.length===0&&!(node.flags&ts.NodeFlags.OptionalChain)&&ts.isIdentifier(unwrapRead(node.expression))){
      const binding=bindings.get(checker.getSymbolAtLocation(unwrapRead(node.expression)));
      if(binding){
        const chain=functions(node),callback=chain[0],memo=chain.find(fn=>fn.parent&&native(fn.parent,'createMemo')&&fn.parent.arguments[0]===fn);
        if(memo&&callback!==memo){
          const expression=returned(node,callback),intent=chain.some(fn=>fn.parent&&native(fn.parent,'untrack'));
          if(!expression||intent)open.push({start:node.getStart(source),reason:intent?'exact native untrack expresses deliberate read intent':'read does not flow through an admitted returned expression'});
          else{
            const start=node.getStart(source),position=source.getLineAndCharacterOfPosition(start),creatorSymbol=core.get(native(binding.creation,'createSignal')?'createSignal':'createMemo');
            sites.push({kind:'memo-callback-read',path:source.fileName,sourceSha256:hash(source.text),start,end:node.end,line:position.line+1,column:position.character+1,
              accessorBinding:{start:binding.binding.getStart(source),end:binding.binding.end},
              creation:{start:binding.creation.getStart(source),end:binding.creation.end,declarations:creatorSymbol.declarations.map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),sha256:hash(d.getSourceFile().text)}))},
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
