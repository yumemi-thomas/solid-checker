// Rewrite native async bodies without adding awaits, promise handlers or owners.
import {ts} from './lower.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v2.mjs';
export function transformAsyncContinuations(code,path,session){
  const state=session.get(path,code),{program,source,errors}=state;
  const context={generation:state.generation,reused:state.reused,validationMs:state.validationMs,buildMs:state.buildMs,inputCount:state.inputCount,revision:state.revision};
  if(errors.length)return {code,map:null,sites:[],helpers:[],open:[{reason:'TypeScript owns this input',codes:errors.map(error=>error.code)}],session:context};
  const candidates=nativeReadSites(program,source),async=asyncContinuationSites(program,source),sites=candidates.sites,helpers=async.functions,open=[...candidates.open,...async.open];
  if(!sites.length&&!helpers.length)return {code,map:null,sites,helpers,open,session:context};
  const names=new Set();function namesIn(node){if(ts.isIdentifier(node))names.add(node.text);ts.forEachChild(node,namesIn);}namesIn(source);
  let prefix='__packageAsyncObservation';while([...names].some(name=>name.startsWith(prefix)))prefix+='_';
  const f=ts.factory,id=suffix=>f.createIdentifier(prefix+suffix),selected=new Map(sites.map(site=>[site.start+':'+site.end,site])),enrolled=new Map(helpers.map(helper=>[helper.function.start+':'+helper.function.end,helper]));
  const method=(name,args)=>f.createCallExpression(id(name),undefined,args),literal=value=>f.createStringLiteral(JSON.stringify(value)),arrow=expression=>f.createArrowFunction(undefined,undefined,[],undefined,f.createToken(ts.SyntaxKind.EqualsGreaterThanToken),expression);
  const original=(next,node)=>{ts.setOriginalNode(next,node);return ts.setTextRange(next,node);};
  const emitted=ts.transpileModule(code,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,jsx:ts.JsxEmit.Preserve,sourceMap:true,inlineSources:true},transformers:{before:[context=>root=>{
    let counter=0;
    function visit(node,active=null){
      if(ts.isFunctionLike(node)){
        const helper=enrolled.get(node.getStart(root)+':'+node.end);
        if(!helper)return ts.visitEachChild(node,child=>visit(child,null),context);
        const token=id('Token'+counter++),metadata={...helper,projectRevision:state.revision},operations=new Map(helper.operations.map(operation=>[operation.start+':'+operation.end,operation]));
        const nextActive={token,helper:metadata,operations};
        let body=ts.isBlock(node.body)?ts.visitEachChild(node.body,child=>visit(child,nextActive),context):f.createBlock([f.createReturnStatement(method('Return',[token,ts.visitNode(node.body,child=>visit(child,nextActive))]))],true);
        const statements=[...body.statements],directives=[];
        while(statements[0]&&ts.isExpressionStatement(statements[0])&&ts.isStringLiteral(statements[0].expression))directives.push(statements.shift());
        const entry=statements[0]??node.body;
        const capture=original(f.createVariableStatement(undefined,f.createVariableDeclarationList([f.createVariableDeclaration(token,undefined,undefined,method('Capture',[literal(metadata)]))],ts.NodeFlags.Const)),entry);
        const error=id('Error'+counter++),caught=f.createCatchClause(f.createVariableDeclaration(error),f.createBlock([f.createExpressionStatement(method('Fail',[token])),f.createThrowStatement(error)],true));
        const final=f.createBlock([f.createExpressionStatement(method('Finish',[token]))],true);
        const block=f.createBlock([...directives,capture,f.createTryStatement(f.createBlock(statements,true),caught,final)],true);
        if(ts.isFunctionDeclaration(node))return f.updateFunctionDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,block);
        if(ts.isFunctionExpression(node))return f.updateFunctionExpression(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,block);
        if(ts.isArrowFunction(node))return f.updateArrowFunction(node,node.modifiers,node.typeParameters,node.parameters,node.type,node.equalsGreaterThanToken,block);
        if(ts.isMethodDeclaration(node))return f.updateMethodDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.questionToken,node.typeParameters,node.parameters,node.type,block);
        return node;
      }
      if(active&&ts.isReturnStatement(node))return original(f.updateReturnStatement(node,method('Return',[active.token,node.expression?visit(node.expression,active):f.createVoidZero()])),node);
      const site=selected.get(node.getStart(root)+':'+node.end),operation=active?.operations.get(node.getStart(root)+':'+node.end);
      if((site||operation)&&(ts.isCallExpression(node)||ts.isPropertyAccessExpression(node))){
        const next=ts.visitEachChild(node,child=>visit(child,active),context);
        return original(site?method('Read',[literal({...site,projectRevision:state.revision}),arrow(next)]):method('Continue',[active.token,literal(operation),arrow(next)]),node);
      }
      return ts.visitEachChild(node,child=>visit(child,active),context);
    }
    const next=ts.visitNode(root,node=>visit(node)),bindings=[['withNativeReadCandidate','Read'],['captureAsyncContinuation','Capture'],['withAsyncContinuation','Continue'],['asyncContinuationReturn','Return'],['asyncContinuationFail','Fail'],['asyncContinuationFinish','Finish']];
    const imported=f.createImportDeclaration(undefined,f.createImportClause(false,undefined,f.createNamedImports(bindings.map(([name,suffix])=>f.createImportSpecifier(false,f.createIdentifier(name),id(suffix))))),f.createStringLiteral('/@fs'+new URL('./native-read-runtime-v5.mjs',import.meta.url).pathname));
    return f.updateSourceFile(next,[imported,...next.statements]);
  }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),sites,helpers,open,session:context};
}
