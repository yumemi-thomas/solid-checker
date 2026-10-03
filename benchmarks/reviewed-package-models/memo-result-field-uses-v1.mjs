// Constant callback fields do not explain consumer use of object identity or
// effects. Limit guidance filtering to exact direct reads of those data fields.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
function wrappedParent(node){const parent=node.parent;return parent&&parent.expression===node&&(ts.isParenthesizedExpression(parent)||ts.isAsExpression(parent)||ts.isTypeAssertionExpression(parent)||ts.isNonNullExpression(parent)||ts.isSatisfiesExpression(parent))?parent:null;}
export function memoResultFieldUses(program,source,site,fields){
  const checker=program.getTypeChecker(),candidates=[];
  function locate(node){if(node.getStart(source)===site.memo.start&&node.end===site.memo.end&&ts.isCallExpression(node))candidates.push(node);ts.forEachChild(node,locate);}locate(source);
  if(candidates.length!==1)return null;let expression=candidates[0];while(wrappedParent(expression))expression=expression.parent;
  const declaration=expression.parent;
  if(!ts.isVariableDeclaration(declaration)||declaration.initializer!==expression||!ts.isIdentifier(declaration.name)||!(declaration.parent.flags&ts.NodeFlags.Const)||ts.isSourceFile(declaration.parent.parent.parent))return null;
  const symbol=checker.getSymbolAtLocation(declaration.name);if(symbol?.declarations?.length!==1||symbol.declarations[0]!==declaration)return null;
  const keys=new Set(fields.map(row=>row.key)),uses=[];let refused=false;
  function visit(node){
    if(ts.isShorthandPropertyAssignment(node)&&checker.getShorthandAssignmentValueSymbol(node)===symbol)refused=true;
    if(ts.isIdentifier(node)&&node!==declaration.name&&checker.getSymbolAtLocation(node)===symbol){
      let reference=node;while(wrappedParent(reference))reference=reference.parent;
      const call=reference.parent;
      if(!ts.isCallExpression(call)||call.expression!==reference||call.arguments.length||call.questionDotToken||call.flags&ts.NodeFlags.OptionalChain){refused=true;return;}
      let result=call;while(wrappedParent(result))result=result.parent;
      const property=result.parent;
      if(!ts.isPropertyAccessExpression(property)||property.expression!==result||property.questionDotToken||property.flags&ts.NodeFlags.OptionalChain||!keys.has(property.name.text)||ts.isAssignmentTarget(property)||ts.isDeleteExpression(property.parent)||ts.isTaggedTemplateExpression(property.parent)&&property.parent.tag===property){refused=true;return;}
      uses.push({path:source.fileName,start:property.getStart(source),end:property.end,key:property.name.text,sha256:hash(source.text)});
    }
    ts.forEachChild(node,visit);
  }visit(source);
  if(refused||!uses.length)return null;
  return {kind:'source-local-memo-direct-data-field-uses',declaration:{path:source.fileName,start:declaration.getStart(source),end:declaration.end,sha256:hash(source.text)},uses,
    scope:'written references in the exact local consumer source',objectIdentity:'open',effects:'open',mutationAfterReturn:'open',authority:false,certification:false};
}
