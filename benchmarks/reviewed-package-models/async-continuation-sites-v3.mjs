// Exact lexical callback bodies supplement async bodies; no scheduling model.
import {asyncContinuationSites as preceding} from './async-continuation-sites-v2.mjs';
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
export function asyncContinuationSites(program,source){
  const base=preceding(program,source),functions=[...base.functions],open=[...base.open],checker=program.getTypeChecker();
  const span=node=>({path:source.fileName,start:node.getStart(source),end:node.end,sha256:hash(source.text)}),target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  // Preserve the original whole member call, including a computed receiver.
  // Its declaration is exact; runtime dispatch and scheduling remain unproved.
  const suspends=node=>!ts.isFunctionLike(node)&&(ts.isAwaitExpression(node)||ts.isYieldExpression(node)||!!ts.forEachChild(node,child=>suspends(child)||undefined));
  for(const helper of base.functions){let fn;function locate(node){if(ts.isFunctionLike(node)&&node.getStart(source)===helper.function.start&&node.end===helper.function.end)fn=node;ts.forEachChild(node,locate);}locate(source);
    function operations(node){if(node!==fn.body&&ts.isFunctionLike(node))return;if(ts.isCallExpression(node)&&ts.isPropertyAccessExpression(unwrapRead(node.expression))&&!(node.flags&ts.NodeFlags.OptionalChain)&&!suspends(node)&&!node.arguments.some(ts.isSpreadElement)&&!helper.operations.some(row=>row.start===node.getStart(source)&&row.end===node.end)){const expression=unwrapRead(node.expression),symbol=target(checker.getSymbolAtLocation(expression.name));if(symbol?.declarations?.length)helper.operations.push({...span(node),kind:'call',declarations:symbol.declarations.map(node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)}))});}ts.forEachChild(node,operations);}operations(fn.body);
  }
  function visit(fn){
    if((ts.isArrowFunction(fn)||ts.isFunctionExpression(fn))&&fn.body&&!fn.asteriskToken&&!fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword)&&ts.isCallExpression(fn.parent)&&fn.parent.arguments.includes(fn)){
      let enclosing=fn.parent;while(enclosing&&!ts.isFunctionLike(enclosing))enclosing=enclosing.parent;
      const parent=base.functions.find(row=>enclosing&&row.function.start===enclosing.getStart(source)&&row.function.end===enclosing.end),operation=parent?.operations.find(row=>row.start===fn.parent.getStart(source)&&row.end===fn.parent.end);
      if(parent&&operation){const operations=[];let refused=false;
        function own(node){if(node!==fn.body&&ts.isFunctionLike(node))return;
          if(ts.isAwaitExpression(node)||ts.isYieldExpression(node))refused=true;
          if(ts.isCallExpression(node)){const callee=unwrapRead(node.expression),symbol=ts.isIdentifier(callee)||ts.isPropertyAccessExpression(callee)?target(checker.getSymbolAtLocation(ts.isPropertyAccessExpression(callee)?callee.name:callee)):null;
            if(node.flags&ts.NodeFlags.OptionalChain||node.arguments.some(ts.isSpreadElement)||!symbol?.declarations?.length)refused=true;
            else if(symbol.declarations.some(declaration=>program.isSourceFileDefaultLibrary(declaration.getSourceFile())&&ts.isFunctionDeclaration(declaration)&&declaration.name?.text==='eval'))refused=true;
            else operations.push({...span(node),kind:'call',declarations:symbol.declarations.map(node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)}))});
          }ts.forEachChild(node,own);
        }own(fn.body);
        if(!refused&&operations.length)functions.push({kind:'source-lexical-callback-helper',function:span(fn),lexicalParent:parent.function,creationOperation:operation,operations,authority:false,certification:false});
        else open.push({function:span(fn),reason:'lexical callback operations remain unresolved or reference-sensitive'});
      }
    }ts.forEachChild(fn,visit);
  }visit(source);return {functions,open};
}
