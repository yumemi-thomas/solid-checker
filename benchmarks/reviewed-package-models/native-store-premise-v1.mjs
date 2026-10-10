// Exact native store target/Proxy and own-data serving symbols, tied to bytes.
import assert from 'node:assert/strict';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {nativeAccessorArtifact} from './native-accessor-hook-v1.mjs';
export function nativeStorePremise(text,path){
  assert.equal(hash(text),nativeAccessorArtifact,'Store artifact is outside the inspected profile');
  const options={allowJs:true,noResolve:true,noLib:true,target:ts.ScriptTarget.ESNext},host={...ts.createCompilerHost(options),getSourceFile(file){return file===path?ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS):undefined;}};
  const program=ts.createProgram([path],options,host),source=program.getSourceFile(path),checker=program.getTypeChecker();assert.equal(source.parseDiagnostics.length,0);
  const constructor=source.statements.find(node=>ts.isFunctionDeclaration(node)&&node.name.text==='createTarget');assert(constructor);
  const final=constructor.body.statements.at(-1);assert(ts.isReturnStatement(final)&&ts.isIdentifier(final.expression));
  const targetSymbol=checker.getSymbolAtLocation(final.expression);assert.equal(targetSymbol.declarations.length,1);const binding=targetSymbol.declarations[0];assert(ts.isVariableDeclaration(binding));
  const proxy=constructor.body.statements.find(node=>ts.isExpressionStatement(node)&&ts.isBinaryExpression(node.expression)&&ts.isNewExpression(node.expression.right));assert(proxy);
  const expression=proxy.expression.right;assert.equal(expression.arguments.length,2);assert.equal(checker.getSymbolAtLocation(expression.arguments[0]),targetSymbol);
  assert(ts.isIdentifier(expression.expression)&&expression.expression.text==='Proxy');assert.equal(checker.getSymbolAtLocation(expression.expression)?.declarations?.length??0,0,'Native Proxy must not be shadowed');
  const trapSymbol=checker.getSymbolAtLocation(expression.arguments[1]);assert.equal(trapSymbol.declarations.length,1);const traps=trapSymbol.declarations[0];assert(ts.isVariableDeclaration(traps)&&ts.isObjectLiteralExpression(traps.initializer));
  const getter=traps.initializer.properties.find(node=>ts.isMethodDeclaration(node)&&node.name.text==='get');assert(getter);
  const finalGet=getter.body.statements.at(-1);assert(ts.isReturnStatement(finalGet)&&ts.isCallExpression(finalGet.expression));
  const readerSymbol=checker.getSymbolAtLocation(finalGet.expression.expression);assert.equal(readerSymbol.declarations.length,1);const reader=readerSymbol.declarations[0];assert(ts.isFunctionDeclaration(reader));
  for(const index of [0,1])assert.equal(checker.getSymbolAtLocation(finalGet.expression.arguments[index]),checker.getSymbolAtLocation(getter.parameters[index].name));
  assert.equal(reader.parameters.length,6);const returns=[];function collect(node){if(node!==reader&&ts.isFunctionLike(node))return;if(ts.isReturnStatement(node))returns.push(node);ts.forEachChild(node,collect);}collect(reader);assert(returns.length>0);
  const span=node=>({start:node.getStart(source),end:node.end}),position=source.getLineAndCharacterOfPosition(final.expression.getStart());
  return {kind:'native-store-target-and-data-read',path,sourceSha256:hash(text),line:position.line+1,column:position.character+1,
    constructor:span(constructor),binding:span(binding),proxy:span(expression),returned:span(final.expression),traps:span(traps),getter:span(getter),dispatch:span(finalGet),
    reader:span(reader),entry:span(reader.body.statements[0]),returns:returns.map(span),
    names:{target:reader.parameters[0].name.text,key:reader.parameters[1].name.text,reader:reader.name.text},staticDispatch:'open',authority:false,certification:false};
}
