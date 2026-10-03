// One exact retained native artifact, with an independently checkable hook site.
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {hash,packageRoot,read} from './catalog.mjs';
export const nativeAccessorArtifact='sha256:f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120';

export function nativeAccessorPremise(text,path){
  assert.equal(hash(text),nativeAccessorArtifact,'Native accessor artifact is outside the inspected profile');
  const options={allowJs:true,noResolve:true,noLib:true,target:ts.ScriptTarget.ESNext};
  const host={...ts.createCompilerHost(options),getSourceFile(file){return file===path?ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS):undefined;}};
  const program=ts.createProgram([path],options,host),source=program.getSourceFile(path),checker=program.getTypeChecker();
  assert.equal(source.parseDiagnostics.length,0);
  const exports=checker.getExportsOfModule(checker.getSymbolAtLocation(source));
  function exported(name){const symbol=exports.find(item=>item.getName()===name);assert(symbol,name);const target=symbol.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;assert.equal(target.declarations.length,1);return target.declarations[0];}
  const signal=exported('createSignal'),memo=exported('createMemo');assert(ts.isFunctionDeclaration(signal)&&ts.isFunctionDeclaration(memo));
  const returned=memo.body.statements.find(ts.isReturnStatement);assert(returned&&ts.isCallExpression(returned.expression)&&ts.isIdentifier(returned.expression.expression));
  const accessorSymbol=checker.getSymbolAtLocation(returned.expression.expression);assert.equal(accessorSymbol.declarations.length,1);const accessor=accessorSymbol.declarations[0];assert(ts.isFunctionDeclaration(accessor));
  const parameter=accessor.parameters[0];assert.equal(accessor.parameters.length,1);assert(ts.isIdentifier(parameter.name));
  const binding=accessor.body.statements[0].declarationList.declarations[0];assert(ts.isVariableDeclaration(binding)&&ts.isIdentifier(binding.name));
  const init=binding.initializer;assert(ts.isCallExpression(init)&&ts.isPropertyAccessExpression(init.expression)&&init.expression.name.text==='bind');
  assert.equal(init.arguments.length,2);assert.equal(init.arguments[0].kind,ts.SyntaxKind.NullKeyword);
  assert.equal(checker.getSymbolAtLocation(init.arguments[1]),checker.getSymbolAtLocation(parameter.name));
  const boundReader=checker.getSymbolAtLocation(init.expression.expression);assert(boundReader?.declarations?.some(ts.isImportSpecifier));
  const final=accessor.body.statements.at(-1);assert(ts.isReturnStatement(final)&&ts.isIdentifier(final.expression));
  assert.equal(checker.getSymbolAtLocation(final.expression),checker.getSymbolAtLocation(binding.name));
  let signalReturns=0;function scan(node){if(node!==signal&&ts.isFunctionLike(node))return;
    if(ts.isReturnStatement(node)){assert(ts.isArrayLiteralExpression(node.expression)&&node.expression.elements.length===2);const getter=node.expression.elements[0];assert(ts.isCallExpression(getter));assert.equal(checker.getSymbolAtLocation(getter.expression),accessorSymbol);signalReturns++;}
    ts.forEachChild(node,scan);}
  scan(signal);assert.equal(signalReturns,2);
  for(const name of ['getObserver','getOwner'])assert(exports.some(item=>item.getName()===name));
  const position=source.getLineAndCharacterOfPosition(final.expression.getStart(source));
  const span=node=>({start:node.getStart(source),end:node.end});
  return {kind:'native-bound-reactive-reader',path,sourceSha256:hash(text),line:position.line+1,column:position.character+1,
    accessor:span(accessor),binding:span(binding),returned:span(final.expression),factories:{createSignal:span(signal),createMemo:span(memo)},
    staticDispatch:'open',authority:false,certification:false};
}

export function instrumentNativeAccessor(text,path,{runtimeSpecifier=new URL('./native-identity-runtime-v1.mjs',import.meta.url).href}={}){
  const premise=nativeAccessorPremise(text,path),f=ts.factory;
  const emitted=ts.transpileModule(text,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,sourceMap:true,inlineSources:true},
    transformers:{before:[context=>source=>{
      function visit(node){if(node.getStart(source)===premise.returned.start&&node.end===premise.returned.end){
        const hook=f.createPropertyAccessChain(f.createPropertyAccessExpression(f.createIdentifier('globalThis'),'__nativeAccessorIdentities'),f.createToken(ts.SyntaxKind.QuestionDotToken),'tag');
        const call=f.createCallChain(hook,undefined,undefined,[node,f.createStringLiteral(JSON.stringify(premise)),f.createIdentifier('getObserver'),f.createIdentifier('getOwner')]);
        const result=f.createBinaryExpression(call,f.createToken(ts.SyntaxKind.QuestionQuestionToken),node);ts.setOriginalNode(result,node);return ts.setTextRange(result,node);
      }return ts.visitEachChild(node,visit,context);}
      const next=ts.visitNode(source,visit);
      return f.updateSourceFile(next,[f.createImportDeclaration(undefined,undefined,f.createStringLiteral(runtimeSpecifier)),...next.statements]);
    }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),premise};
}

export function nativeAccessorHook(){const transformed=[],refused=[];
  return {transformed,refused,transform(code,path){
    if(!path.endsWith('/dist/dev.js'))return null;
    let root;try{root=packageRoot(dirname(path),'@solidjs/signals');}catch{return null;}
    if(realpathSync(path)!==realpathSync(join(root,'dist/dev.js')))return null;
    try{assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');assert.equal(readFileSync(path,'utf8'),code);
      const result=instrumentNativeAccessor(code,path,{runtimeSpecifier:'/@fs'+new URL('./native-identity-runtime-v1.mjs',import.meta.url).pathname});transformed.push(result.premise);return {code:result.code,map:result.map};
    }catch(error){refused.push({path,reason:error.message});return null;}
  }};
}
