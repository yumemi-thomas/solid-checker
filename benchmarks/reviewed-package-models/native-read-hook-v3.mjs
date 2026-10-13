// Exact shared reader and untrack artifacts; no behavior inferred from API names.
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {hash,packageRoot,read} from './catalog.mjs';
import {nativeAccessorPremise} from './native-accessor-hook-v1.mjs';
import {nativeStorePremise} from './native-store-premise-v1.mjs';
export const nativeSharedArtifact='sha256:70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e';
export function nativeReaderPremise(text,path){
  assert.equal(hash(text),nativeSharedArtifact,'Shared reader artifact is outside the inspected profile');
  const options={allowJs:true,noResolve:true,noLib:true,target:ts.ScriptTarget.ESNext},host={...ts.createCompilerHost(options),getSourceFile(file){return file===path?ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS):undefined;}};
  const program=ts.createProgram([path],options,host),source=program.getSourceFile(path),checker=program.getTypeChecker();assert.equal(source.parseDiagnostics.length,0);
  const exports=checker.getExportsOfModule(checker.getSymbolAtLocation(source));
  function exported(name){const symbol=exports.find(item=>item.getName()===name);assert(symbol);const target=symbol.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;assert.equal(target.declarations.length,1);const declaration=target.declarations[0];assert(ts.isFunctionDeclaration(declaration));return declaration;}
  const reader=exported('r'),untrack=exported('b0'),observer=exported('aX'),owner=exported('g');assert.equal(reader.parameters.length,1);assert(ts.isIdentifier(reader.parameters[0].name));
  const returns=[];function visit(node){if(node!==reader&&ts.isFunctionLike(node))return;if(ts.isReturnStatement(node))returns.push(node);ts.forEachChild(node,visit);}visit(reader);assert(returns.length>0);
  const span=node=>({start:node.getStart(source),end:node.end});
  return {kind:'native-reader-and-untrack',path,sourceSha256:hash(text),reader:span(reader),parameter:span(reader.parameters[0].name),
    entry:span(reader.body.statements[0]),returns:returns.map(span),untrack:span(untrack),observer:span(observer),owner:span(owner),
    names:{reader:reader.name.text,parameter:reader.parameters[0].name.text,observer:observer.name.text,owner:owner.name.text},
    staticDispatch:'open',authority:false,certification:false};
}
export function instrumentNativeReads(text,path,{runtimeSpecifier=new URL('./native-read-runtime-v3.mjs',import.meta.url).href}={}){
  const shared=path.endsWith('/dev-shared.js'),premise=shared?nativeReaderPremise(text,path):nativeAccessorPremise(text,path),f=ts.factory,store=shared?null:nativeStorePremise(text,path);
  const source=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS),names=new Set();function namesIn(node){if(ts.isIdentifier(node))names.add(node.text);ts.forEachChild(node,namesIn);}namesIn(source);
  let prefix='__observedNativeReads';while([...names].some(name=>name.startsWith(prefix)))prefix+='_';const id=suffix=>f.createIdentifier(prefix+suffix);
  const trace=()=>f.createPropertyAccessExpression(f.createIdentifier('globalThis'),'__nativeNodeReads');
  const decl=(name,initializer)=>f.createVariableStatement(undefined,f.createVariableDeclarationList([f.createVariableDeclaration(name,undefined,undefined,initializer)],ts.NodeFlags.Const));
  const method=(name,args)=>f.createCallExpression(f.createPropertyAccessExpression(id('Trace'),name),undefined,args);
  const emitted=ts.transpileModule(text,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,sourceMap:true,inlineSources:true},
    transformers:{before:[context=>root=>{
      function visit(node){
        if(!shared&&node.getStart(root)===premise.returned.start&&node.end===premise.returned.end){
          const accessor=root.statements.find(n=>ts.isFunctionDeclaration(n)&&n.getStart(root)===premise.accessor.start);assert(accessor);const parameter=accessor.parameters[0].name;
          const hook=f.createPropertyAccessChain(trace(),f.createToken(ts.SyntaxKind.QuestionDotToken),'tag'),call=f.createCallChain(hook,undefined,undefined,[node,parameter,f.createStringLiteral(JSON.stringify(premise))]);
          const result=f.createBinaryExpression(call,f.createToken(ts.SyntaxKind.QuestionQuestionToken),node);ts.setOriginalNode(result,node);return ts.setTextRange(result,node);
        }
        if(!shared&&node.getStart(root)===store.returned.start&&node.end===store.returned.end){
          const call=f.createCallChain(f.createPropertyAccessChain(trace(),f.createToken(ts.SyntaxKind.QuestionDotToken),'tagStore'),undefined,undefined,[node,f.createStringLiteral(JSON.stringify(store))]);
          const result=f.createBinaryExpression(call,f.createToken(ts.SyntaxKind.QuestionQuestionToken),node);ts.setOriginalNode(result,node);return ts.setTextRange(result,node);
        }
        const dataReader=!shared&&ts.isFunctionDeclaration(node)&&node.getStart(root)===store.reader.start;
        if(dataReader||shared&&ts.isFunctionDeclaration(node)&&node.getStart(root)===premise.reader.start){
          const entry=node.body.statements[0];
          function returned(n){if(n!==node.body&&ts.isFunctionLike(n))return n;
            if(ts.isReturnStatement(n)){
              const expression=n.expression??f.createVoidZero(),next=f.updateReturnStatement(n,f.createConditionalExpression(id('Trace'),undefined,method('finish',[expression,id('Ticket')]),undefined,expression));
              ts.setOriginalNode(next,n);return ts.setTextRange(next,n);
            }return ts.visitEachChild(n,returned,context);}
          const body=ts.visitNode(node.body,returned),traceDecl=decl(id('Trace'),trace()),ticketDecl=decl(id('Ticket'),f.createConditionalExpression(id('Trace'),undefined,
            dataReader?method('beginStore',[f.createIdentifier(store.names.target),f.createIdentifier(store.names.key),f.createStringLiteral(JSON.stringify(store)),f.createIdentifier('getObserver'),f.createIdentifier('getOwner')]):method('begin',[f.createIdentifier(premise.names.parameter),f.createStringLiteral(JSON.stringify(premise)),f.createIdentifier(premise.names.observer),f.createIdentifier(premise.names.owner)]),undefined,f.createNull()));
          for(const generated of [traceDecl,ticketDecl]){ts.setOriginalNode(generated,entry);ts.setTextRange(generated,entry);}
          return f.updateFunctionDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,f.updateBlock(body,[traceDecl,ticketDecl,...body.statements]));
        }
        if(shared&&ts.isFunctionDeclaration(node)&&node.getStart(root)===premise.untrack.start){
          const traceDecl=decl(id('Trace'),trace()),previous=decl(id('Intent'),f.createConditionalExpression(id('Trace'),undefined,method('enterIntent',[]),undefined,f.createNumericLiteral(0))),
            final=f.createBlock([f.createIfStatement(id('Trace'),f.createExpressionStatement(method('leaveIntent',[id('Intent')])))],true);
          const wrapped=f.createTryStatement(node.body,undefined,final);
          return f.updateFunctionDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,f.createBlock([traceDecl,previous,wrapped],true));
        }
        return ts.visitEachChild(node,visit,context);
      }
      const next=ts.visitNode(root,visit);return f.updateSourceFile(next,[f.createImportDeclaration(undefined,undefined,f.createStringLiteral(runtimeSpecifier)),...next.statements]);
    }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),premise,store};
}
export function nativeReadHook(){const transformed=[],refused=[];
  return {transformed,refused,transform(code,path){
    if(!path.endsWith('/dist/dev.js')&&!path.endsWith('/dist/dev-shared.js'))return null;
    let root;try{root=packageRoot(dirname(path),'@solidjs/signals');}catch{return null;}
    if(realpathSync(path)!==realpathSync(join(root,'dist',path.endsWith('/dev-shared.js')?'dev-shared.js':'dev.js')))return null;
    try{assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');assert.equal(readFileSync(path,'utf8'),code);
      const result=instrumentNativeReads(code,path,{runtimeSpecifier:'/@fs'+new URL('./native-read-runtime-v3.mjs',import.meta.url).pathname});transformed.push(result.premise);if(result.store)transformed.push(result.store);return {code:result.code,map:result.map};
    }catch(error){refused.push({path,reason:error.message});return null;}
  }};
}

