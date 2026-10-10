import { ts } from './lower.mjs';
export function instrumentCallbackArguments(args, mode) {
  const text = `candidate(${args.join(', ')});`, source = ts.createSourceFile('callbacks.ts', text, ts.ScriptTarget.Latest, true), edits = []; let count = 0;
  function visit(node) {
    if (ts.isArrowFunction(node)) {
      if (ts.isBlock(node.body)) throw new Error('The generated witness requires an expression callback');
      const id = count++;
      const prefix = `{ if (h.samples.length < 512) h.samples.push({ id: ${id}, tracked: !!getObserver(), owned: !!getOwner(), stage: h.stage }); else h.saturated = true; h.lastRead = read(); ${mode === 'write' ? 'set(2);' : ''} return (`;
      edits.push({ position: node.body.getStart(source), text: prefix }, { position: node.body.end, text: '); }' });
    }
    ts.forEachChild(node, visit);
  }
  visit(source); let output = text;
  for (const edit of edits.sort((a,b) => b.position-a.position)) output = output.slice(0, edit.position) + edit.text + output.slice(edit.position);
  const parsed = ts.createSourceFile('callbacks.ts', output, ts.ScriptTarget.Latest, true);
  if (parsed.parseDiagnostics.length) throw new Error('Callback instrumentation must remain syntactically valid');
  return { count, arguments: parsed.statements[0].expression.arguments.map(node => node.getText(parsed)) };
}
