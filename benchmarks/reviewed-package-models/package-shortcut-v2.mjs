// Exact source enrollment for an observed branch; this does not prove a defect.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {ts} from './lower.mjs';
import {signalAccessorUses} from './signal-accessor-use-v1.mjs';
import {hash,nativeRuntimeRoots,read} from './catalog.mjs';
const unwrap=node=>{while(ts.isParenthesizedExpression(node))node=node.expression;return node;};
export function packageShortcuts(text,path){
  const options={allowJs:true,noEmit:true,noLib:true,target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,customConditions:['browser','development']};
  const host=ts.createCompilerHost(options),original=host.getSourceFile;host.getSourceFile=(file,...args)=>file===path?ts.createSourceFile(file,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS):original(file,...args);
  const program=ts.createProgram([path],options,host),source=program.getSourceFile(path),checker=program.getTypeChecker();assert.equal(source.parseDiagnostics.length,0);
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  const providers=new Map(),imports=new Map(),span=node=>({start:node.getStart(source),end:node.end});
  const pins=symbol=>(symbol?.declarations??[]).map(d=>({path:d.getSourceFile().fileName,start:d.getStart(),end:d.end,sha256:hash(d.getSourceFile().text)}));
  for(const statement of source.statements)if(ts.isImportDeclaration(statement)&&['solid-js','@solidjs/signals'].includes(statement.moduleSpecifier.text)&&statement.importClause?.namedBindings&&ts.isNamedImports(statement.importClause.namedBindings)){
    const module=checker.getSymbolAtLocation(statement.moduleSpecifier);if(!module)continue;
    const exports=checker.getExportsOfModule(module),provider={module:statement.moduleSpecifier.text};
    for(const name of ['getObserver','getOwner','createSignal'])provider[name]=target(exports.find(symbol=>symbol.getName()===name));
    if(!['getObserver','getOwner','createSignal'].every(name=>provider[name]?.declarations?.length))continue;
    providers.set(provider.module,provider);
    for(const item of statement.importClause.namedBindings.elements){const symbol=checker.getSymbolAtLocation(item.name),name=(item.propertyName??item.name).text;if(provider[name]&&target(symbol)===provider[name])imports.set(symbol,{name,provider});}
  }
  const exactCall=(node,name)=>{node=unwrap(node);if(!ts.isCallExpression(node)||!ts.isIdentifier(node.expression))return null;const imported=imports.get(checker.getSymbolAtLocation(node.expression));return imported?.name===name&&target(checker.getSymbolAtLocation(node.expression))===imported.provider[name]?imported:null;};
  const models=[];
  function visit(node){
    if(ts.isIfStatement(node)){
      const condition=unwrap(node.expression),call=ts.isPrefixUnaryExpression(condition)&&condition.operator===ts.SyntaxKind.ExclamationToken?unwrap(condition.operand):null,observer=call&&exactCall(call,'getObserver');
      const returned=ts.isReturnStatement(node.thenStatement)?node.thenStatement:ts.isBlock(node.thenStatement)&&node.thenStatement.statements.length===1&&ts.isReturnStatement(node.thenStatement.statements[0])?node.thenStatement.statements[0]:null;
      let fn=node.parent;while(fn&&!ts.isFunctionLike(fn))fn=fn.parent;
      if(observer&&call.arguments.length===0&&returned&&fn?.body&&!fn.asteriskToken&&!fn.modifiers?.some(m=>m.kind===ts.SyntaxKind.AsyncKeyword)){
        const factories=[];function scan(child){if(child!==fn.body&&ts.isFunctionLike(child))return;if(child.getStart(source)>returned.end&&exactCall(child,'createSignal'))factories.push(child);ts.forEachChild(child,scan);}scan(fn.body);
        const used=factories.map(factory=>({factory,uses:signalAccessorUses(program,source,fn,factory)})).filter(item=>item.uses.length);
        if(used.length){const position=source.getLineAndCharacterOfPosition(returned.getStart(source));models.push({kind:'package-observer-shortcut',path,sourceSha256:hash(text),line:position.line+1,column:position.character+1,
          guard:span(node),observerCall:span(call),returned:span(returned),function:span(fn),signalPaths:used.map(item=>span(item.factory)),accessorUses:used.map(item=>item.uses),module:observer.provider.module,
          observerDeclarations:pins(observer.provider.getObserver),ownerDeclarations:pins(observer.provider.getOwner),signalDeclarations:used.map(item=>pins(exactCall(item.factory,'createSignal').provider.createSignal)),
          behavior:'observed no-observer return before a source-enrolled accessor use or object escape',valueFlow:'open',staticDispatch:'open',authority:false,certification:false});}
      }
    }ts.forEachChild(node,visit);
  }visit(source);return {models,program,source};
}
export function instrumentPackageShortcuts(text,path){
  const {models,source}=packageShortcuts(text,path);if(!models.length)return null;
  const f=ts.factory,names=new Set();function identifiers(node){if(ts.isIdentifier(node))names.add(node.text);ts.forEachChild(node,identifiers);}identifiers(source);
  let prefix='__observedPackageShortcut';while([...names].some(name=>name.startsWith(prefix)))prefix+='_';
  const modules=[...new Set(models.map(model=>model.module))],id=suffix=>f.createIdentifier(prefix+suffix),selected=new Map(models.map(model=>[model.returned.start,model]));
  const decl=(name,value)=>f.createVariableStatement(undefined,f.createVariableDeclarationList([f.createVariableDeclaration(name,undefined,undefined,value)],ts.NodeFlags.Const));
  const emitted=ts.transpileModule(text,{fileName:path,compilerOptions:{target:ts.ScriptTarget.ESNext,module:ts.ModuleKind.ESNext,sourceMap:true,inlineSources:true},transformers:{before:[context=>root=>{
    function visit(node){const model=selected.get(node.getStart(root));
      if(model&&ts.isReturnStatement(node)&&node.end===model.returned.end){
        const trace=decl(id('Trace'),f.createPropertyAccessExpression(f.createIdentifier('globalThis'),'__nativeNodeReads'));
        const begin=f.createCallExpression(f.createPropertyAccessExpression(id('Trace'),'beginGuard'),undefined,[f.createStringLiteral(JSON.stringify(model)),id('Owner'+modules.indexOf(model.module))]);
        const ticket=decl(id('Ticket'),f.createConditionalExpression(id('Trace'),undefined,begin,undefined,f.createNull()));
        const value=node.expression??f.createVoidZero(),result=f.createReturnStatement(f.createConditionalExpression(id('Trace'),undefined,f.createCallExpression(f.createPropertyAccessExpression(id('Trace'),'finish'),undefined,[value,id('Ticket')]),undefined,value));
        for(const generated of [trace,ticket,result]){ts.setOriginalNode(generated,node);ts.setTextRange(generated,node);}
        const block=f.createBlock([trace,ticket,result],true);ts.setOriginalNode(block,node);return ts.setTextRange(block,node);
      }return ts.visitEachChild(node,visit,context);
    }
    const next=ts.visitNode(root,visit),injected=new Set(),statements=next.statements.map(statement=>{
      if(!ts.isImportDeclaration(statement)||!modules.includes(statement.moduleSpecifier.text)||injected.has(statement.moduleSpecifier.text)||!statement.importClause?.namedBindings||!ts.isNamedImports(statement.importClause.namedBindings))return statement;
      const module=statement.moduleSpecifier.text;injected.add(module);const specifier=f.createImportSpecifier(false,f.createIdentifier('getOwner'),id('Owner'+modules.indexOf(module)));
      return f.updateImportDeclaration(statement,statement.modifiers,f.updateImportClause(statement.importClause,statement.importClause.isTypeOnly,statement.importClause.name,f.updateNamedImports(statement.importClause.namedBindings,[...statement.importClause.namedBindings.elements,specifier])),statement.moduleSpecifier,statement.attributes);
    });assert.equal(injected.size,modules.length);return f.updateSourceFile(next,statements);
  }]}});return {code:emitted.outputText,map:JSON.parse(emitted.sourceMapText),models};
}
export function packageShortcutHook(){const transformed=[],refused=[];
  return {transformed,refused,transform(code,path){
    if(!path.includes('/node_modules/')||!path.endsWith('.js')||path.includes('/solid-js/')||path.includes('/@solidjs/signals/')||!code.includes('getObserver'))return null;
    try{assert.equal(readFileSync(path,'utf8'),code,'Original package bytes required');for(const root of nativeRuntimeRoots(dirname(path)))assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');
      const result=instrumentPackageShortcuts(code,path);if(!result)return null;transformed.push(...result.models);return {code:result.code,map:result.map};
    }catch(error){refused.push({path,reason:error.message});return null;}
  }};
}

