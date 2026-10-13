// Keep probe sources owned, and invoke the package later from an imperative
// scope. The callback's implementation and package arguments stay identical.
import assert from 'node:assert/strict';
import cases from './callback-cases.mjs';
import { ts } from './lower.mjs';
export default cases.map(entry=>{
  const source=ts.createSourceFile('consumer.tsx',entry.source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const statement=source.statements.find(s=>ts.isExpressionStatement(s)&&ts.isBinaryExpression(s.expression)&&ts.isPropertyAccessExpression(s.expression.left)&&s.expression.left.name.text==='dispose');
  const call=statement.expression.right; assert(ts.isCallExpression(call)&&call.expression.text==='createRoot');
  const body=call.arguments[0].body;
  const split=body.statements.findIndex(s=>ts.isVariableStatement(s)&&s.declarationList.declarations[0].name.text==='invoke'); assert(split>0);
  assert(ts.isReturnStatement(body.statements.at(-1)));
  const prefix=body.statements.slice(0,split).map(s=>s.getText(source)).join('\n'), tail=body.statements.slice(split,-1).map(s=>s.getText(source)).join('\n');
  const replacement=`{ ${prefix} h.invokeLater = () => { ${tail} }; return dispose; }`;
  const text=entry.source.slice(0,body.getStart(source))+replacement+entry.source.slice(body.end);
  const marker="h.attempt('flush', () => flush());"; assert(text.includes(marker));
  return {...entry,id:entry.id+'-imperative',source:text.replace(marker,`h.stage = 'imperative'; h.attempt('invoke-later', h.invokeLater); ${marker}`),
    provenance:{...entry.provenance,callerProfile:'imperative'}};
});
