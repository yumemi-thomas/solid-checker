// Exact source function/operation enrollment; an executed native witness is required.
import {ts} from './lower.mjs';
import {hash} from './catalog.mjs';
import {unwrapRead} from './native-read-sites-v4.mjs';
export function asyncContinuationSites(program,source){
  const checker=program.getTypeChecker(),functions=[],open=[];
  const target=symbol=>symbol?.flags&ts.SymbolFlags.Alias?checker.getAliasedSymbol(symbol):symbol;
  const span=node=>({path:node.getSourceFile().fileName,start:node.getStart(),end:node.end,sha256:hash(node.getSourceFile().text)});
  const simple=node=>{node=unwrapRead(node);return ts.isIdentifier(node)||ts.isPropertyAccessExpression(node)&&!node.questionDotToken&&simple(node.expression);};
  const suspends=node=>!ts.isFunctionLike(node)&&(ts.isAwaitExpression(node)||ts.isYieldExpression(node)||!!ts.forEachChild(node,child=>suspends(child)||undefined));
  const wrapper=node=>ts.isParenthesizedExpression(node)||ts.isAsExpression(node)||ts.isTypeAssertionExpression(node)||ts.isNonNullExpression(node)||ts.isSatisfiesExpression(node);
  function referencePosition(node){
    while(node.parent&&wrapper(node.parent)&&node.parent.expression===node)node=node.parent;
    return ts.isAssignmentTarget(node)||node.parent&&ts.isDeleteExpression(node.parent)||node.parent&&ts.isTaggedTemplateExpression(node.parent)&&node.parent.tag===node;
  }
  function enroll(fn){
    if(!fn.body||!fn.modifiers?.some(modifier=>modifier.kind===ts.SyntaxKind.AsyncKeyword))return;
    if(fn.asteriskToken){open.push({function:span(fn),reason:'async generators stay outside continuation observations'});return;}
    let evalCall=false;const operations=[];
    function visit(node){
      if(node!==fn.body&&ts.isFunctionLike(node))return;
      const call=ts.isCallExpression(node)&&!(node.flags&ts.NodeFlags.OptionalChain),property=ts.isPropertyAccessExpression(node)&&!(node.flags&ts.NodeFlags.OptionalChain)&&simple(node.expression)&&
        !(ts.isCallExpression(node.parent)&&node.parent.expression===node)&&!(ts.isPropertyAccessExpression(node.parent)&&node.parent.expression===node);
      if(call||property){
        const expression=unwrapRead(call?node.expression:node),member=ts.isPropertyAccessExpression(expression)&&simple(expression.expression),symbol=ts.isIdentifier(expression)||member?target(checker.getSymbolAtLocation(member?expression.name:expression)):null;
        if(property&&referencePosition(node))open.push({function:span(fn),operation:span(node),reason:'property assignment, update, delete or tag receiver must retain its reference'});
        else if(call&&ts.isIdentifier(expression)&&symbol?.declarations?.some(d=>program.isSourceFileDefaultLibrary(d.getSourceFile())&&ts.isFunctionDeclaration(d)&&d.name?.text==='eval'))evalCall=true;
        else if(symbol?.declarations?.length&&!(call&&node.arguments.some(suspends)))operations.push({...span(node),kind:call?'call':'property',declarations:symbol.declarations.map(span)});
        else open.push({function:span(fn),operation:span(node),reason:'continuation operation lacks an exact admitted declaration or suspends in its arguments'});
      }
      ts.forEachChild(node,visit);
    }
    visit(fn.body);
    if(evalCall){open.push({function:span(fn),reason:'direct eval cannot move into a continuation observation block'});return;}
    functions.push({kind:'source-async-helper',function:span(fn),operations,authority:false,certification:false});
  }
  function visit(node){if(ts.isFunctionLike(node))enroll(node);ts.forEachChild(node,visit);}visit(source);
  return {functions,open};
}
