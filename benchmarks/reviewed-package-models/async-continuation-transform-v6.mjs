// Keep every original member call; registered callbacks activate at their own entry.
import {ts} from './lower.mjs';
import {nativeReadSites} from './native-read-sites-v4.mjs';
import {asyncContinuationSites} from './async-continuation-sites-v2.mjs';
import {callbackSlotSites} from './callback-slot-sites-v2.mjs';
export function transformAsyncContinuations(code,path,session){
  const state=session.get(path,code),{program,source,errors}=state;
  const context={generation:state.generation,reused:state.reused,validationMs:state.validationMs,buildMs:state.buildMs,inputCount:state.inputCount,revision:state.revision};
  if(errors.length)return {code,map:null,sites:[],helpers:[],callbackSlots:null,open:[{reason:'TypeScript owns this input',codes:errors.map(error=>error.code)}],session:context};
  const candidates=nativeReadSites(program,source),async=asyncContinuationSites(program,source),slots=callbackSlotSites(program,source),sites=candidates.sites,helpers=async.functions,open=[...candidates.open,...async.open,...slots.open];
  if(!sites.length&&!helpers.length&&!slots.allocations.length&&!slots.invocations.length&&!slots.callbacks.length)return {code,map:null,sites,helpers,callbackSlots:slots,open,session:context};
  const names=new Set();function namesIn(node){if(ts.isIdentifier(node))names.add(node.text);ts.forEachChild(node,namesIn);}namesIn(source);
  let prefix='__packageAsyncObservation';while([...names].some(name=>name.startsWith(prefix)))prefix+='_';
  const f=ts.factory,id=suffix=>f.createIdentifier(prefix+suffix),index=(values,get)=>new Map(values.map(value=>{const span=get(value);return [span.start+':'+span.end,value];}));
  const selected=index(sites,x=>x),enrolled=index(helpers,x=>x.function),entries=index(slots.callbacks,x=>x.function),allocations=index(slots.allocations,x=>x.allocation),invocations=index(slots.invocations,x=>x.operation);
  const method=(name,args)=>f.createCallExpression(id(name),undefined,args),literal=value=>f.createStringLiteral(JSON.stringify(value)),arrow=expression=>f.createArrowFunction(undefined,undefined,[],undefined,f.createToken(ts.SyntaxKind.EqualsGreaterThanToken),expression);
  const original=(next,node)=>{ts.setOriginalNode(next,node);return ts.setTextRange(next,node);},rev=value=>({...value,projectRevision:state.revision});
  const emitted=ts.transpileModule(code,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,jsx:ts.JsxEmit.Preserve,sourceMap:true,inlineSources:true},transformers:{before:[context=>root=>{
    let counter=0;
    function visit(node,active=null){
      const key=node.getStart(root)+':'+node.end;
      if(ts.isFunctionLike(node)){
        const helper=enrolled.get(key),entry=entries.get(key);
        if(!helper&&!entry)return ts.visitEachChild(node,child=>visit(child,null),context);
        const token=id('Token'+counter++),box=entry?.creation==='nameless-allocation'?id('Self'+counter++):null;
        const self=box?f.createPropertyAccessExpression(box,'fn'):entry?.self?f.createIdentifier(entry.self.name):null;
        const metadata=rev(helper??entry),nextActive={token,operations:new Map((helper?.operations??[]).map(operation=>[operation.start+':'+operation.end,operation])),returnMethod:helper?'Return':'SlotReturn'};
        let body=ts.isBlock(node.body)?ts.visitEachChild(node.body,child=>visit(child,nextActive),context):f.createBlock([f.createReturnStatement(method(nextActive.returnMethod,[token,ts.visitNode(node.body,child=>visit(child,nextActive))]))],true);
        const statements=[...body.statements],directives=[];
        while(statements[0]&&ts.isExpressionStatement(statements[0])&&ts.isStringLiteral(statements[0].expression))directives.push(statements.shift());
        const capture=original(f.createVariableStatement(undefined,f.createVariableDeclarationList([f.createVariableDeclaration(token,undefined,undefined,method(helper&&entry?'SlotCapture':helper?'Capture':'SlotEnter',helper&&entry?[self,literal(rev(entry)),literal(metadata)]:helper?[literal(metadata)]:[self,literal(metadata)]))],ts.NodeFlags.Const)),statements[0]??node.body);
        const error=id('Error'+counter++),caught=f.createCatchClause(f.createVariableDeclaration(error),f.createBlock([f.createExpressionStatement(method(helper?'Fail':'SlotFail',[token])),f.createThrowStatement(error)],true));
        const final=f.createBlock([f.createExpressionStatement(method(helper?'Finish':'SlotFinish',[token]))],true);
        const block=f.createBlock([...directives,capture,f.createTryStatement(f.createBlock(statements,true),caught,final)],true);
        let next=node;
        if(ts.isFunctionDeclaration(node))next=f.updateFunctionDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,block);
        else if(ts.isFunctionExpression(node))next=f.updateFunctionExpression(node,node.modifiers,node.asteriskToken,node.name,node.typeParameters,node.parameters,node.type,block);
        else if(ts.isArrowFunction(node))next=f.updateArrowFunction(node,node.modifiers,node.typeParameters,node.parameters,node.type,node.equalsGreaterThanToken,block);
        else if(ts.isMethodDeclaration(node))next=f.updateMethodDeclaration(node,node.modifiers,node.asteriskToken,node.name,node.questionToken,node.typeParameters,node.parameters,node.type,block);
        if(!box)return next;
        // Assignment to a property preserves the nameless allocation's empty name.
        return original(f.createCallExpression(arrow(f.createBlock([
          f.createVariableStatement(undefined,f.createVariableDeclarationList([f.createVariableDeclaration(box,undefined,undefined,f.createObjectLiteralExpression([f.createPropertyAssignment('fn',f.createNull())]))],ts.NodeFlags.Const)),
          f.createReturnStatement(f.createAssignment(f.createPropertyAccessExpression(box,'fn'),next)),
        ],true)),undefined,[]),node);
      }
      if(active&&ts.isReturnStatement(node))return original(f.updateReturnStatement(node,method(active.returnMethod,[active.token,node.expression?visit(node.expression,active):f.createVoidZero()])),node);
      const next=ts.visitEachChild(node,child=>visit(child,active),context);
      if(allocations.has(key))return original(method('SlotRegister',[next,literal(rev(allocations.get(key)))]),node);
      let value=next;
      if(invocations.has(key)){const invocation=invocations.get(key);value=method('SlotInvoke',[f.createIdentifier(node.expression.expression.text),f.createStringLiteral(invocation.key),literal(rev(invocation)),arrow(value)]);}
      const site=selected.get(key),operation=active?.operations.get(key);
      if(site)value=method('Read',[literal(rev(site)),arrow(value)]);
      else if(operation)value=method('Continue',[active.token,literal(operation),arrow(value)]);
      return value===next?next:original(value,node);
    }
    const next=ts.visitNode(root,node=>visit(node)),bindings=[['withNativeReadCandidate','Read'],['captureAsyncContinuation','Capture'],['withAsyncContinuation','Continue'],['asyncContinuationReturn','Return'],['asyncContinuationFail','Fail'],['asyncContinuationFinish','Finish']],slotBindings=[['captureRegisteredAsyncCallback','SlotCapture'],['registerCallbackSlots','SlotRegister'],['withCallbackSlotInvocation','SlotInvoke'],['enterRegisteredCallback','SlotEnter'],['registeredCallbackReturn','SlotReturn'],['registeredCallbackFail','SlotFail'],['registeredCallbackFinish','SlotFinish']];
    const imported=(bindings,module)=>f.createImportDeclaration(undefined,f.createImportClause(false,undefined,f.createNamedImports(bindings.map(([name,suffix])=>f.createImportSpecifier(false,f.createIdentifier(name),id(suffix))))),f.createStringLiteral('/@fs'+new URL(module,import.meta.url).pathname));
    return f.updateSourceFile(next,[imported(bindings,'./native-read-runtime-v9.mjs'),imported(slotBindings,'./callback-slot-runtime-v4.mjs'),...next.statements]);
  }]}});
  return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),sites,helpers,callbackSlots:slots,open,session:context};
}
