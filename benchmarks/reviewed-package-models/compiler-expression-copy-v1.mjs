// Positive point-map provenance for an unchanged parsed expression. No range
// interpolation or claim about lexical binding, tracking, or runtime use.
export function exactMappedPoint(decoded,line,column){
  const segments=(decoded[line]??[]).filter(segment=>segment[0]===column);
  if(segments.length!==1||segments[0].length<4||segments[0][1]!==0)return null;
  return {line:segments[0][2],character:segments[0][3]};
}
export function jsxExpressions(ts,source){
  const values=[];function visit(node){if(ts.isJsxExpression(node)&&node.expression)values.push(node.expression);else if(ts.isJsxSpreadAttribute(node))values.push(node.expression);ts.forEachChild(node,visit);}visit(source);return values;
}
export function singleReturnGetters(ts,source){
  const values=[];function visit(node){if(ts.isGetAccessorDeclaration(node)&&node.body?.statements.length===1&&ts.isReturnStatement(node.body.statements[0])&&node.body.statements[0].expression)values.push(node);ts.forEachChild(node,visit);}visit(source);return values;
}
function atoms(ts,node){
  const values=[];function visit(child){if(ts.isIdentifier(child)||ts.isLiteralExpression(child))values.push(child);ts.forEachChild(child,visit);}visit(node);return values;
}
function structure(ts,node){const children=[];ts.forEachChild(node,child=>{children.push(structure(ts,child));});return {kind:node.kind,children};}
function samePoint(left,right){return left&&right&&left.line===right.line&&left.character===right.character;}
export function getterExpressionCopy(ts,source,generated,decoded,expressions,getter){
  const expression=getter.body.statements[0].expression,start=generated.getLineAndCharacterOfPosition(expression.getStart(generated)),mappedStart=exactMappedPoint(decoded,start.line,start.character);
  const candidates=mappedStart?expressions.filter(node=>samePoint(source.getLineAndCharacterOfPosition(node.getStart(source)),mappedStart)):[];
  if(candidates.length!==1)return {status:'open',reason:'source-start-not-exact-and-unique'};
  const original=candidates[0];
  if(original.getText(source)!==expression.getText(generated)||JSON.stringify(structure(ts,original))!==JSON.stringify(structure(ts,expression)))return {status:'open',reason:'expression-copy-differs'};
  const sourceAtoms=atoms(ts,original),generatedAtoms=atoms(ts,expression);
  if(!sourceAtoms.length||sourceAtoms.length!==generatedAtoms.length)return {status:'open',reason:'literal-or-identifier-mappings-unavailable'};
  const atomMappings=[];
  for(const [index,node] of sourceAtoms.entries()){
    const emitted=generatedAtoms[index],point=generated.getLineAndCharacterOfPosition(emitted.getStart(generated)),mapped=exactMappedPoint(decoded,point.line,point.character),originalPoint=source.getLineAndCharacterOfPosition(node.getStart(source));
    if(node.kind!==emitted.kind||node.getText(source)!==emitted.getText(generated)||!samePoint(originalPoint,mapped))return {status:'open',reason:'literal-or-identifier-mapping-differs'};
    atomMappings.push({kind:node.kind,text:node.getText(source),sourcePoint:originalPoint,generatedPoint:point});
  }
  const end=generated.getLineAndCharacterOfPosition(expression.end),mappedEnd=exactMappedPoint(decoded,end.line,end.character),originalEnd=source.getLineAndCharacterOfPosition(original.end);
  return {status:'copy-witness',original,expression,atomMappings,endMapping:samePoint(originalEnd,mappedEnd)?'exact':'unobserved'};
}
