// Separate reconstruction of written consumers; no policy/proof imports.
import assert from 'node:assert/strict';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
export function auditMemoResultFieldUses(program,source,site,model,fields){
  assert.equal(model.kind,'source-local-memo-direct-data-field-uses');assert.equal(model.scope,'written references in the exact local consumer source');
  for(const key of ['objectIdentity','effects','mutationAfterReturn'])assert.equal(model[key],'open');assert.equal(model.authority,false);assert.equal(model.certification,false);
  const checker=program.getTypeChecker(),strip=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isNonNullExpression(node)||ts.isTypeAssertionExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
  const nodes=[];function inventory(node){nodes.push(node);ts.forEachChild(node,inventory);}inventory(source);
  const calls=nodes.filter(node=>ts.isCallExpression(node)&&node.getStart(source)===site.memo.start&&node.end===site.memo.end);assert.equal(calls.length,1);
  const declarations=nodes.filter(node=>ts.isVariableDeclaration(node)&&strip(node.initializer)===calls[0]);assert.equal(declarations.length,1);const declaration=declarations[0];
  assert(ts.isIdentifier(declaration.name));assert(declaration.parent.flags&ts.NodeFlags.Const);assert(!ts.isSourceFile(declaration.parent.parent.parent));
  assert.deepEqual(model.declaration,{path:source.fileName,start:declaration.getStart(source),end:declaration.end,sha256:hash(source.text)});
  const symbol=checker.getSymbolAtLocation(declaration.name);assert.deepEqual(symbol.declarations,[declaration]);const allowed=new Set(fields.map(field=>field.key)),expected=[];
  for(const node of nodes){
    if(ts.isShorthandPropertyAssignment(node))assert.notEqual(checker.getShorthandAssignmentValueSymbol(node),symbol);
    if(!ts.isIdentifier(node)||node===declaration.name||checker.getSymbolAtLocation(node)!==symbol)continue;
    const uses=nodes.filter(candidate=>ts.isCallExpression(candidate)&&strip(candidate.expression)===node);assert.equal(uses.length,1);const call=uses[0];assert.equal(call.arguments.length,0);assert(!call.questionDotToken&&!(call.flags&ts.NodeFlags.OptionalChain));
    const reads=nodes.filter(candidate=>ts.isPropertyAccessExpression(candidate)&&strip(candidate.expression)===call);assert.equal(reads.length,1);const property=reads[0];assert(!property.questionDotToken&&!(property.flags&ts.NodeFlags.OptionalChain));assert(allowed.has(property.name.text));
    assert(!ts.isAssignmentTarget(property));assert(!ts.isDeleteExpression(property.parent));assert(!(ts.isTaggedTemplateExpression(property.parent)&&property.parent.tag===property));
    expected.push({path:source.fileName,start:property.getStart(source),end:property.end,key:property.name.text,sha256:hash(source.text)});
  }
  assert(expected.length);assert.deepEqual(model.uses,expected);return {declaration:model.declaration,uses:expected.length,scope:model.scope};
}
