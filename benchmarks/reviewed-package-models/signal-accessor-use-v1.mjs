// Local source facts for accessor use/escape; no external dispatch is asserted.
import {ts} from './lower.mjs';
const unwrap=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
export function signalAccessorUses(program,source,fn,factory){
  const checker=program.getTypeChecker(),span=node=>({start:node.getStart(source),end:node.end}),nodes=[];
  function collect(node){if(node!==fn.body&&ts.isFunctionLike(node))return;nodes.push(node);ts.forEachChild(node,collect);}collect(fn.body);
  const symbol=node=>ts.isIdentifier(unwrap(node))?checker.getSymbolAtLocation(unwrap(node)):null;
  const declaration=target=>target?.declarations?.length===1?target.declarations[0]:null;
  const local=node=>node?.getSourceFile()===source&&node.getStart(source)>=fn.getStart(source)&&node.end<=fn.end;
  const resolveAlias=(node,base,seen=new Set())=>{
    node=unwrap(node);const target=symbol(node);if(!target||seen.has(target))return null;if(target===base)return [];
    const binding=declaration(target);if(!local(binding)||!ts.isVariableDeclaration(binding)||!(binding.parent.flags&ts.NodeFlags.Const)||!binding.initializer)return null;
    const previous=resolveAlias(binding.initializer,base,new Set([...seen,target]));return previous?[...previous,span(binding)]:null;
  };
  let expression=factory;while(expression.parent&&unwrap(expression.parent)===factory)expression=expression.parent;
  const parent=expression.parent,uses=[];
  function record(kind,binding,use,aliases=[]){uses.push({kind,factory:span(factory),binding:binding?span(binding):null,use:span(use),aliasBindings:aliases,dispatch:'open',resultFlow:'open'});}
  if(ts.isVariableDeclaration(parent)&&parent.initializer===expression&&ts.isArrayBindingPattern(parent.name)&&parent.parent.flags&ts.NodeFlags.Const){
    const first=parent.name.elements[0];
    if(first&&ts.isBindingElement(first)&&ts.isIdentifier(first.name)&&!first.dotDotDotToken&&!first.initializer){
      const getter=symbol(first.name);
      for(const node of nodes){if(node.getStart(source)<=factory.end)continue;
        if(ts.isCallExpression(node)&&node.arguments.length===0&&!node.questionDotToken){const aliases=resolveAlias(node.expression,getter);if(aliases)record('accessor-call',first,node,aliases);}
        if(ts.isShorthandPropertyAssignment(node)&&checker.getShorthandAssignmentValueSymbol(node)===getter)record('accessor-object-escape',first,node);
        if(ts.isPropertyAssignment(node)&&(ts.isIdentifier(node.name)||ts.isStringLiteral(node.name)||ts.isNumericLiteral(node.name))){const aliases=resolveAlias(node.initializer,getter);if(aliases)record('accessor-object-escape',first,node,aliases);}
      }
    }
  }
  // A directly called first tuple member needs no local binding.
  if(ts.isElementAccessExpression(parent)&&parent.expression===expression&&ts.isNumericLiteral(parent.argumentExpression)&&parent.argumentExpression.text==='0'&&ts.isCallExpression(parent.parent)&&parent.parent.expression===parent&&parent.parent.arguments.length===0&&!parent.parent.questionDotToken)
    record('inline-index-call',null,parent.parent);
  const assigned=[];
  if(ts.isVariableDeclaration(parent)&&ts.isIdentifier(parent.name)&&parent.initializer===expression)assigned.push(symbol(parent.name));
  let current=expression;
  while(current.parent&&ts.isBinaryExpression(current.parent)&&current.parent.operatorToken.kind===ts.SyntaxKind.EqualsToken&&current.parent.right===current){
    const target=symbol(current.parent.left);if(target)assigned.push(target);current=current.parent;
  }
  for(const tuple of assigned){const binding=declaration(tuple);if(!binding||!local(binding)||!ts.isVariableDeclaration(binding))continue;
    for(const call of nodes.filter(ts.isCallExpression)){
      const indexed=unwrap(call.expression);if(call.getStart(source)<=factory.end||call.arguments.length||call.questionDotToken||!ts.isElementAccessExpression(indexed)||indexed.questionDotToken||!ts.isNumericLiteral(indexed.argumentExpression)||indexed.argumentExpression.text!=='0')continue;
      const aliases=resolveAlias(indexed.expression,tuple);if(!aliases)continue;
      // Explicit rebinding after this factory destroys even the local origin fact.
      const rebound=nodes.some(node=>{
        if(node.getStart(source)<=factory.end||node.getStart(source)>=call.getStart(source)||!ts.isBinaryExpression(node)||!ts.isAssignmentOperator(node.operatorToken.kind))return false;
        const left=unwrap(node.left);return symbol(left)===tuple||(ts.isElementAccessExpression(left)||ts.isPropertyAccessExpression(left))&&resolveAlias(left.expression,tuple)!==null;
      });
      if(!rebound)record('tuple-index-call',binding,call,aliases);
    }
  }
  return uses.sort((a,b)=>a.use.start-b.use.start);
}
