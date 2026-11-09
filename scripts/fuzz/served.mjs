// What the Chrome host reads of a case's served (compiled) modules to put a frame on the program's text (chrome-host.mjs `mapPositions`).
// Part of the host's record key (`chromeHostKey`), so a change here makes the records again.
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ts = createRequire(join(resolve(dirname(fileURLToPath(import.meta.url)), "../../packages/cli"), "package.json"))("typescript");

/**
 * A module of the program besides App.tsx (`helpers.ts`, `el.ts`): a top-level `.ts`/`.tsx` source, not a declaration file, the
 * checker's entry (main.tsx) or the Chrome host's (sc-entry.ts). Frames and findings in one are placed in that module, never App.tsx.
 */
export const isProgramModule = (file) => typeof file === "string" && /^[^/]+\.tsx?$/.test(file) && !file.endsWith(".d.ts") && !["App.tsx", "main.tsx", "sc-entry.ts"].includes(file);

/** The dev compiler's Solid runtime calls that create a computation for one expression they are handed, and the argument holding it. */
const COMPILED_HANDED = { insert: 1, effect: 0, memo: 0 };
/**
 * ADR 0921: `(line, column) => [{ line, column }]` over a served module's code (the dev compiler's output; 1-based, generated positions):
 * where the callee of a compiled `insert(el, value)`, `effect(() => value, …)` or `memo(() => value)` of @solidjs/web holds the position,
 * the start of the `value` it was handed, through the compiler's arrows (`() => value`, `() => { return value; }`) and `readShallow`. The compiler maps such a call to its
 * element, attribute or fragment (`<p>`, `class=`, `<>`), never to the expression the computation runs: V8's frame lands there, while a
 * checker row is on the expression. The value's own start, mapped back by the caller, is that expression's.
 */
export function compiledHanded(code) {
  const text = String(code ?? "");
  const file = ts.createSourceFile("served.js", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const imported = new Map();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement) || !/@solidjs[/_]web/.test(statement.moduleSpecifier.text)) continue;
    const named = statement.importClause?.namedBindings;
    if (named && ts.isNamedImports(named)) for (const element of named.elements) imported.set(element.name.text, element.propertyName?.text ?? element.name.text);
  }
  const unwrapValue = (node) => {
    let inner = node;
    for (;;) {
      if (ts.isParenthesizedExpression(inner)) inner = inner.expression;
      else if (ts.isArrowFunction(inner) && !ts.isBlock(inner.body)) inner = inner.body;
      else if (ts.isArrowFunction(inner) && inner.body.statements.length === 1 && ts.isReturnStatement(inner.body.statements[0]) && inner.body.statements[0].expression) inner = inner.body.statements[0].expression;
      else if (ts.isCallExpression(inner) && ts.isIdentifier(inner.expression) && imported.get(inner.expression.text) === "readShallow" && inner.arguments.length === 1) inner = inner.arguments[0];
      else return inner;
    }
  };
  const calls = [];
  const visit = (node) => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && Object.hasOwn(COMPILED_HANDED, imported.get(node.expression.text) ?? "")) {
      const value = node.arguments[COMPILED_HANDED[imported.get(node.expression.text)]];
      if (value) calls.push([node.expression.getStart(file), node.expression.getEnd(), unwrapValue(value).getStart(file)]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lineStarts = file.getLineStarts();
  return (line, column) => {
    const offset = (lineStarts[line - 1] ?? Infinity) + column - 1;
    return calls.filter(([from, to]) => from <= offset && offset <= to).map(([, , at]) => { const p = file.getLineAndCharacterOfPosition(at); return { line: p.line + 1, column: p.character + 1 }; });
  };
}

