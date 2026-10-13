// Derived protocol experiments, not fresh semantic challenges.
import queue from '../noise-zero-replay-cases-v1.mjs';
import revisions from '../feedback-revision-cases-v2.mjs';
import assert from 'node:assert/strict';
import {ts} from '../lower.mjs';
function withPendingProbe(code){
  const source=ts.createSourceFile('main.tsx',code,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let binding,use;
  function visit(node){
    if(ts.isVariableDeclaration(node)&&ts.isIdentifier(node.name)&&node.name.text==='result')binding=node;
    if(ts.isCallExpression(node)&&ts.isIdentifier(node.expression)&&node.expression.text==='result'&&!node.arguments.length){
      use=ts.isPropertyAccessExpression(node.parent)&&node.parent.expression===node?node.parent:node;
    }
    ts.forEachChild(node,visit);
  }visit(source);assert(binding&&use&&binding.parent.parent.end<use.getStart());
  const end=binding.parent.parent.end;
  return code.slice(0,end)+`h.probePending=()=>h.checkPending(()=>${use.getText(source)});`+code.slice(end);
}
export default [...queue,...revisions].map(row=>({...row,
  source:withPendingProbe(row.source),
  completion:row.id.includes('never-settling-promise')?'authored-body-counter':'authored-pending-probe',
  artifactOrigin:'derived-authored-readiness-protocol',
}));
