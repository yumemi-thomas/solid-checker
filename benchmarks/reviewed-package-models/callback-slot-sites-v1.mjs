// Allocation/entry facts are exact; callback timing needs runtime identity evidence.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
export function callbackSlotSites(program,source){
  const checker=program.getTypeChecker(),allocations=[],invocations=[],callbacks=[],open=[],symbol=node=>checker.getSymbolAtLocation(node),span=node=>({path:source.fileName,start:node.getStart(source),end:node.end,sha256:hash(source.text)});
  const ancestors=node=>{const result=[];for(let p=node.parent;p;p=p.parent)if(ts.isFunctionLike(p))result.push(p);return result;};
  const writes=new Set();let evalCall=false;
  function absence(node){if(ts.isIdentifier(node)&&ts.isAssignmentTarget(node)){const s=symbol(node);if(s)writes.add(s);}if(ts.isCallExpression(node)&&ts.isIdentifier(unwrapRead(node.expression))){const s=symbol(unwrapRead(node.expression)),target=s?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(s):s;if(target?.declarations?.some(d=>program.isSourceFileDefaultLibrary(d.getSourceFile())&&ts.isFunctionDeclaration(d)&&d.name?.text==='eval'))evalCall=true;}ts.forEachChild(node,absence);}absence(source);
  function parameter(node,seen=new Set()){
    node=unwrapRead(node);if(!ts.isIdentifier(node))return null;const s=symbol(node);if(!s||seen.has(s)||writes.has(s)||s.declarations?.length!==1)return null;const d=s.declarations[0];
    if(ts.isParameter(d)&&ts.isIdentifier(d.name)&&d.getSourceFile()===source)return {declaration:span(d),aliases:[]};
    if(d.getSourceFile()===source&&ts.isVariableDeclaration(d)&&d.parent.flags&ts.NodeFlags.Const&&d.initializer){const p=parameter(d.initializer,new Set([...seen,s]));return p?{...p,aliases:[...p.aliases,span(d)]}:null;}return null;
  }
  function key(node){return ts.isIdentifier(node)||ts.isStringLiteral(node)||ts.isNumericLiteral(node)?node.text:null;}
  function shadows(fn,name){let found=fn.parameters.some(p=>p.name.getText(source)===name);function visit(node){if(node!==fn.body&&ts.isFunctionLike(node))return;if((ts.isVariableDeclaration(node)||ts.isClassDeclaration(node))&&node.name?.getText(source)===name)found=true;ts.forEachChild(node,visit);}visit(fn.body);return found;}
  function callback(fn){
    if(!fn.body||fn.asteriskToken||fn.modifiers?.some(m=>m.kind===ts.SyntaxKind.AsyncKeyword)||!(ts.isFunctionDeclaration(fn)||ts.isFunctionExpression(fn)||ts.isArrowFunction(fn)))return;
    let self=null,creation='existing-binding';
    if(fn.name&&ts.isIdentifier(fn.name)&&!writes.has(symbol(fn.name))&&!shadows(fn,fn.name.text))self={name:fn.name.text,declaration:span(fn.name)};
    else if(ts.isVariableDeclaration(fn.parent)&&fn.parent.initializer===fn&&ts.isIdentifier(fn.parent.name)&&fn.parent.parent.flags&ts.NodeFlags.Const&&!writes.has(symbol(fn.parent.name))&&!shadows(fn,fn.parent.name.text))self={name:fn.parent.name.text,declaration:span(fn.parent)};
    else if(!fn.name&&(ts.isReturnStatement(fn.parent)&&fn.parent.expression===fn||ts.isCallExpression(fn.parent)&&fn.parent.arguments.includes(fn))){creation='nameless-allocation';}
    else return;
    callbacks.push({kind:'source-synchronous-callback-entry',function:span(fn),creation,self,authority:false,certification:false});
  }
  function visit(node){
    if(ts.isObjectLiteralExpression(node)){
      const keys=new Set(),fields=[];let admitted=true;
      for(const property of node.properties){if(!(ts.isPropertyAssignment(property)||ts.isShorthandPropertyAssignment(property))||property.objectAssignmentInitializer){admitted=false;break;}const name=key(property.name);if(name===null||name==='__proto__'||keys.has(name)){admitted=false;break;}keys.add(name);let value=ts.isPropertyAssignment(property)?property.initializer:property.name;
        let fact;if(ts.isShorthandPropertyAssignment(property)){const target=checker.getShorthandAssignmentValueSymbol(property),d=target?.declarations?.length===1?target.declarations[0]:null;if(d&&ts.isParameter(d)&&ts.isIdentifier(d.name)&&!writes.has(target))fact={declaration:span(d),aliases:[]};}else fact=parameter(value);
        if(fact&&ancestors(node).some(fn=>fn.parameters.some(p=>p.getStart()===fact.declaration.start&&p.end===fact.declaration.end)))fields.push({key:name,property:span(property),parameter:fact.declaration,aliases:fact.aliases});
      }
      if(admitted&&fields.length)allocations.push({kind:'source-parameter-data-slot',allocation:span(node),fields,authority:false,certification:false});
    }
    if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(node.expression)&&!node.questionDotToken&&!node.expression.questionDotToken&&ts.isIdentifier(node.expression.expression)){
      const receiver=node.expression.expression,s=symbol(receiver),d=s?.declarations?.length===1?s.declarations[0]:null;
      const stable=d&&d.getSourceFile()===source&&!writes.has(s)&&(ts.isVariableDeclaration(d)&&d.parent.flags&ts.NodeFlags.Const||ts.isParameter(d)&&ts.isIdentifier(d.name));
      const suspends=n=>!ts.isFunctionLike(n)&&(ts.isAwaitExpression(n)||ts.isYieldExpression(n)||!!ts.forEachChild(n,c=>suspends(c)||undefined));
      if(stable&&!node.arguments.some(suspends))invocations.push({kind:'source-named-slot-invocation',operation:span(node),key:node.expression.name.text,receiver:span(receiver),declaration:span(d),authority:false,certification:false});
    }
    if(ts.isFunctionLike(node))callback(node);ts.forEachChild(node,visit);
  }
  if(evalCall){open.push({path:source.fileName,reason:'exact direct eval leaves callback slot instrumentation open'});return {allocations,invocations,callbacks,open};}
  visit(source);return {allocations,invocations,callbacks,open};
}
