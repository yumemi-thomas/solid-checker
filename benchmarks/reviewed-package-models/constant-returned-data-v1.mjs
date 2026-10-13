// Narrow source fact: constant own fields in every normal async body return.
// Object identity, Promise adoption, mutation after return and effects stay open.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
import {primitiveConstant} from './constant-call-result-v1.mjs';
export function constantReturnedData(program,metadata){
  const source=program.getSourceFile(metadata?.path);if(!source||hash(source.text)!==metadata.sha256)return null;
  const found=[];function locate(node){if(node.getStart(source)===metadata.start&&node.end===metadata.end&&ts.isFunctionLike(node))found.push(node);ts.forEachChild(node,locate);}locate(source);
  if(found.length!==1)return null;const fn=found[0];
  if(!fn.body||fn.asteriskToken||!fn.modifiers?.some(node=>node.kind===ts.SyntaxKind.AsyncKeyword))return null;
  const checker=program.getTypeChecker();let dynamic=false;
  function dynamicCode(node){if(ts.isCallExpression(node)&&ts.isIdentifier(unwrapRead(node.expression))){let symbol=checker.getSymbolAtLocation(unwrapRead(node.expression));if(symbol?.flags&ts.SymbolFlags.Alias)symbol=checker.getAliasedSymbol(symbol);if(symbol?.declarations?.some(declaration=>program.isSourceFileDefaultLibrary(declaration.getSourceFile())&&ts.isFunctionDeclaration(declaration)&&declaration.name?.text==='eval'))dynamic=true;}ts.forEachChild(node,dynamicCode);}dynamicCode(source);if(dynamic)return null;
  const span=node=>({path:source.fileName,start:node.getStart(source),end:node.end,sha256:hash(source.text)}),returns=[],certificate=[];
  function data(node){
    if(!node)return null;node=unwrapRead(node);if(!ts.isObjectLiteralExpression(node)||!node.properties.length||node.properties.length>16)return null;
    const fields=[],keys=new Set();
    for(const property of node.properties){
      if(!ts.isPropertyAssignment(property))return null;const name=property.name;
      if(!(ts.isIdentifier(name)||ts.isStringLiteral(name)||ts.isNumericLiteral(name)))return null;
      const key=name.text;if(keys.has(key)||['__proto__','then'].includes(key))return null;keys.add(key);
      const constant=primitiveConstant(property.initializer);if(!constant)return null;
      fields.push({key,constant});
    }
    return {kind:'fresh-own-data-fields',fields:fields.sort((a,b)=>a.key.localeCompare(b.key))};
  }
  let visits=0,exhausted=false;const normal={kind:'normal'},thrown={kind:'throw'},unknown={kind:'open'};
  function unique(items){const result=[...new Map(items.map(item=>[JSON.stringify(item),item])).values()];if(result.length>64){exhausted=true;return [unknown];}return result;}
  function flow(node){
    if(++visits>256){exhausted=true;return [unknown];}let result;
    if(ts.isBlock(node)){result=[normal];for(const statement of node.statements){const next=flow(statement);result=unique(result.flatMap(item=>item.kind==='normal'?next:[item]));}}
    else if(ts.isReturnStatement(node)){const value=data(node.expression);const exit={kind:'return',data:value,origin:span(node)};returns.push({span:span(node),expression:node.expression?span(node.expression):null,data:value});result=value?[exit]:[exit,thrown];}
    else if(ts.isThrowStatement(node))result=[thrown];
    else if(ts.isIfStatement(node))result=unique([...flow(node.thenStatement),...(node.elseStatement?flow(node.elseStatement):[normal]),thrown]);
    else if(ts.isTryStatement(node)){result=flow(node.tryBlock);if(node.catchClause){const caught=flow(node.catchClause.block);result=unique(result.flatMap(item=>item.kind==='throw'?caught:[item]));}if(node.finallyBlock){const final=flow(node.finallyBlock);result=unique(result.flatMap(item=>final.map(exit=>exit.kind==='normal'?item:exit)));}}
    else if(ts.isVariableStatement(node)||ts.isExpressionStatement(node)||ts.isClassDeclaration(node))result=[normal,thrown];
    else if(ts.isEmptyStatement(node)||ts.isDebuggerStatement(node)||ts.isFunctionDeclaration(node)||ts.isInterfaceDeclaration(node)||ts.isTypeAliasDeclaration(node))result=[normal];
    else result=[normal,thrown,unknown];
    certificate.push({node:span(node),syntaxKind:ts.SyntaxKind[node.kind],completions:result});return result;
  }
  const completions=ts.isBlock(fn.body)?flow(fn.body):[{kind:'return',data:data(fn.body),origin:span(fn.body)}];
  if(!ts.isBlock(fn.body))returns.push({span:span(fn.body),expression:span(fn.body),data:data(fn.body)});
  const normalReturns=completions.filter(item=>item.kind!=='throw');if(exhausted||!normalReturns.length||normalReturns.some(item=>item.kind!=='return'||!item.data))return null;
  const value=normalReturns[0].data;if(normalReturns.some(item=>JSON.stringify(item.data)!==JSON.stringify(value)))return null;
  return {kind:'source-constant-own-data-body-return',function:span(fn),data:value,returns,certificate,completions,
    scope:'own-field-initializers-on-normal-body-return',promiseSettlement:'unobserved',objectIdentity:'unproved',effects:'open',mutationAfterReturn:'open',bounds:{statements:256,completions:64,fields:16},authority:false,certification:false};
}
