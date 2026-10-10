// Necessary syntax gate for bare and declared member calls; no semantic trust.
import {ts} from './lower.mjs';
import {unwrapRead} from './async-read-sites-v2.mjs';
export function mayHaveNativeCallbackRead(code,path){
  const source=ts.createSourceFile(path,code,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  if(!source.statements.some(node=>ts.isImportDeclaration(node)&&node.moduleSpecifier.text==='solid-js'))return false;
  let possible=false;
  function visit(node){
    if(possible)return;
    const expression=ts.isCallExpression(node)?unwrapRead(node.expression):null;
    if(ts.isPropertyAccessExpression(node)||expression&&(ts.isIdentifier(expression)||ts.isPropertyAccessExpression(expression))){
      let nearest=null;for(let parent=node.parent;parent;parent=parent.parent)if(ts.isFunctionLike(parent)){
        if(!nearest)nearest=parent;else if(ts.isCallExpression(parent.parent)&&parent.parent.arguments[0]===parent){possible=true;return;}
      }
    }
    ts.forEachChild(node,visit);
  }
  visit(source);return possible;
}
