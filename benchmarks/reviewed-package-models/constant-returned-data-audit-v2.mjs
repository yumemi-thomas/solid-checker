// Independent normal-outcome reconstruction; no proof or projector imports.
import assert from 'node:assert/strict';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
export function auditConstantReturnedData(program,model,observation){
  assert.equal(model.kind,'source-constant-own-data-body-return');assert.equal(model.scope,'own-field-initializers-on-normal-body-return');assert.equal(model.promiseSettlement,'unobserved');assert.equal(model.objectIdentity,'unproved');assert.equal(model.effects,'open');assert.equal(model.mutationAfterReturn,'open');assert.equal(model.authority,false);assert.equal(model.certification,false);
  const registration=observation.callbackRegistration;assert.equal(registration.definition.kind,'source-async-callback-entry');assert.equal(registration.returnedKind,'object');assert.deepEqual(registration.definition.function,model.function);
  const source=program.getSourceFile(model.function.path);assert(source);assert.equal(hash(source.text),model.function.sha256);
  const exact=(start,end,predicate=()=>true)=>{const found=[];function visit(node){if(node.getStart(source)===start&&node.end===end&&predicate(node))found.push(node);ts.forEachChild(node,visit);}visit(source);assert.equal(found.length,1);return found[0];};
  const fn=exact(model.function.start,model.function.end,ts.isFunctionLike);assert(ts.isFunctionLike(fn)&&fn.body&&!fn.asteriskToken&&fn.modifiers?.some(node=>node.kind===ts.SyntaxKind.AsyncKeyword));
  const strip=node=>{while(node&&(ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node)))node=node.expression;return node;};
  function literal(node){node=strip(node);if(!node)return null;
    if(ts.isNumericLiteral(node))return {kind:'number',value:String(Number(node.text))};
    if(ts.isStringLiteral(node)||ts.isNoSubstitutionTemplateLiteral(node))return {kind:'string',value:node.text};
    if(node.kind===ts.SyntaxKind.NullKeyword)return {kind:'null',value:null};
    if(node.kind===ts.SyntaxKind.TrueKeyword||node.kind===ts.SyntaxKind.FalseKeyword)return {kind:'boolean',value:node.kind===ts.SyntaxKind.TrueKeyword};
    if(ts.isBigIntLiteral(node))return {kind:'bigint',value:BigInt(node.text.slice(0,-1)).toString()};
    if(ts.isVoidExpression(node)&&ts.isNumericLiteral(strip(node.expression)))return {kind:'undefined',value:null};
    if(ts.isPrefixUnaryExpression(node)&&[ts.SyntaxKind.PlusToken,ts.SyntaxKind.MinusToken].includes(node.operator)&&ts.isNumericLiteral(strip(node.operand))){const n=Number(strip(node.operand).text)*(node.operator===ts.SyntaxKind.MinusToken?-1:1);return {kind:'number',value:Object.is(n,-0)?'-0':String(n)};}return null;
  }
  function data(node){node=strip(node);if(!node||!ts.isObjectLiteralExpression(node)||node.properties.length<1||node.properties.length>16)return null;
    const fields=[],keys=new Set();for(const property of node.properties){if(!ts.isPropertyAssignment(property)||!(ts.isIdentifier(property.name)||ts.isStringLiteral(property.name)||ts.isNumericLiteral(property.name)))return null;const key=property.name.text;if(keys.has(key)||key==='then'||key==='__proto__')return null;keys.add(key);const constant=literal(property.initializer);if(!constant)return null;fields.push({key,constant});}return {kind:'fresh-own-data-fields',fields:fields.sort((a,b)=>a.key.localeCompare(b.key))};
  }
  const checker=program.getTypeChecker();function dynamic(node){if(ts.isCallExpression(node)&&ts.isIdentifier(strip(node.expression))){let symbol=checker.getSymbolAtLocation(strip(node.expression));if(symbol?.flags&ts.SymbolFlags.Alias)symbol=checker.getAliasedSymbol(symbol);assert(!symbol?.declarations?.some(declaration=>program.isSourceFileDefaultLibrary(declaration.getSourceFile())&&ts.isFunctionDeclaration(declaration)&&declaration.name?.text==='eval'));}ts.forEachChild(node,dynamic);}dynamic(source);
  const outcome=(falls,throws,values=[])=>({falls,throws,values:new Map(values.map(value=>[JSON.stringify(value),value]))});
  const merge=(a,b)=>outcome(a.falls||b.falls,a.throws||b.throws,[...a.values.values(),...b.values.values()]);let nodes=0;
  function paths(node){assert(++nodes<=512);
    if(ts.isBlock(node)){let result=outcome(true,false);for(const statement of node.statements){const next=paths(statement);if(result.falls)result=outcome(next.falls,result.throws||next.throws,[...result.values.values(),...next.values.values()]);}return result;}
    if(ts.isReturnStatement(node)){const value=data(node.expression);return outcome(false,!value,[value]);}
    if(ts.isThrowStatement(node))return outcome(false,true);
    if(ts.isIfStatement(node)){const result=merge(paths(node.thenStatement),node.elseStatement?paths(node.elseStatement):outcome(true,false));result.throws=true;return result;}
    if(ts.isTryStatement(node)){let result=paths(node.tryBlock);if(node.catchClause){const caught=paths(node.catchClause.block);if(result.throws)result=outcome(result.falls||caught.falls,caught.throws,[...result.values.values(),...caught.values.values()]);}if(node.finallyBlock){const final=paths(node.finallyBlock);result=outcome(result.falls&&final.falls,final.throws||result.throws&&final.falls,[...final.values.values(),...(final.falls?result.values.values():[])]);}return result;}
    if(ts.isVariableStatement(node)||ts.isExpressionStatement(node)||ts.isClassDeclaration(node))return outcome(true,true);
    if(ts.isEmptyStatement(node)||ts.isDebuggerStatement(node)||ts.isFunctionDeclaration(node)||ts.isInterfaceDeclaration(node)||ts.isTypeAliasDeclaration(node))return outcome(true,false);
    return outcome(true,true,[null]);
  }
  const result=ts.isBlock(fn.body)?paths(fn.body):outcome(false,!data(fn.body),[data(fn.body)]);assert.equal(result.falls,false);assert(result.values.size>0);for(const value of result.values.values())assert.deepEqual(value,model.data);
  for(const item of model.certificate){const node=exact(item.node.start,item.node.end,node=>ts.SyntaxKind[node.kind]===item.syntaxKind);assert.equal(item.node.path,source.fileName);assert.equal(item.node.sha256,hash(source.text));assert.equal(item.syntaxKind,ts.SyntaxKind[node.kind]);}
  return {function:model.function,data:model.data,independentOutcomeNodes:nodes,normalResultAlternatives:result.values.size};
}

