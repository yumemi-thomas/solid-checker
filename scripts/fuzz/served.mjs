// What the Chrome host reads of a case's served (compiled) modules to put a frame on the program's text (chrome-host.mjs `mapPositions`).
// Part of the host's record key (`chromeHostKey`), so a change here makes the records again.
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

// Loaded on first use, never on import (as call-starts.mjs): audit.mjs reaches this module through codes.mjs, and its `pick` and `report`
// jobs run with no install and parse no program.
let loaded = null;
const typescript = () => (loaded ??= createRequire(join(resolve(dirname(fileURLToPath(import.meta.url)), "../../packages/cli"), "package.json"))("typescript"));

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
  const ts = typescript();
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

/**
 * ADR 0930: `(line, column) => [{ line, column }]` over a served module's code (generated positions, 1-based): where a call whose callee
 * is no identifier opens its arguments (`children(f)()`'s second `(`, `(() => x)()`'s), that call's own start. V8 places such a call's
 * frame at that `(` (an identifier callee's at the identifier); the dev compiler's segments past a component's appended dev arguments
 * (`_$createComponent(K, {…}, "K", "App.tsx:8:83"))()`) drift, so the `(` maps to the callee's `)`, the `(` itself or a token inside.
 * The call's start keeps a segment of its own; the caller maps it only where the source map has one exactly there.
 */
export function compiledCallParens(code) {
  const ts = typescript();
  const file = ts.createSourceFile("served.js", String(code ?? ""), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const calls = [];
  const visit = (node) => {
    if (ts.isCallExpression(node) && !node.questionDotToken && !ts.isIdentifier(node.expression)) calls.push([node.arguments.pos - 1, node.getStart(file)]);
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lineStarts = file.getLineStarts();
  return (line, column) => {
    const offset = (lineStarts[line - 1] ?? Infinity) + column - 1;
    return calls.filter(([paren]) => paren === offset).map(([, at]) => { const p = file.getLineAndCharacterOfPosition(at); return { line: p.line + 1, column: p.character + 1 }; });
  };
}

/**
 * The starts of the calls in a module's source (`App.tsx`'s text, 1-based) whose callee is no identifier: where a call the compiler
 * kept starts (`compiledCallParens`), never a synthesized one (an element's IIFE starts at its JSX).
 */
export function sourceCallStarts(source, fileName = "App.tsx") {
  const ts = typescript();
  const file = ts.createSourceFile(fileName, String(source ?? ""), ts.ScriptTarget.Latest, true, fileName.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const starts = new Set();
  const visit = (node) => {
    if (ts.isCallExpression(node) && !node.questionDotToken && !ts.isIdentifier(node.expression)) { const p = file.getLineAndCharacterOfPosition(node.getStart(file)); starts.add(`${p.line + 1}:${p.character + 1}`); }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return ({ line, column }) => starts.has(`${line}:${column}`);
}

/**
 * ADR 0930: `(line, column) => [{ line, column }]` over a served module's code (generated positions, 1-based): where the callee of a
 * compiled `spread(el, props, skipChildren, skip, name, loc)` of @solidjs/web holds the position, the source position the compiler wrote
 * as that call's `loc` (`"cases/<id>/App.tsx:9:22"`: the element's first spread attribute, the location Solid gives the spread's effect),
 * when it names one of `modules` (the module's served paths). The compiler maps such a call to nothing (only its arguments), so V8's frame there has no source position, and the
 * frame below it (the element's IIFE, mapped to the element's end) was taken for the top one. The position is the compiled text's, the
 * instrumented twin's for App.tsx: the caller takes it to the clean text.
 */
export function compiledSpread(code, modules = ["App.tsx"]) {
  const ts = typescript();
  const file = ts.createSourceFile("served.js", String(code ?? ""), ts.ScriptTarget.Latest, true, ts.ScriptKind.JS);
  const spreads = new Set();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement) || !/@solidjs[/_]web/.test(statement.moduleSpecifier.text)) continue;
    const named = statement.importClause?.namedBindings;
    if (named && ts.isNamedImports(named)) for (const element of named.elements) if ((element.propertyName?.text ?? element.name.text) === "spread") spreads.add(element.name.text);
  }
  const calls = [];
  const visit = (node) => {
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && spreads.has(node.expression.text) && node.arguments.length === 6 && ts.isStringLiteral(node.arguments[5])) {
      const at = /^(.+):(\d+):(\d+)$/.exec(node.arguments[5].text);
      if (at && modules.includes(at[1])) calls.push([node.expression.getStart(file), node.expression.getEnd(), { line: Number(at[2]), column: Number(at[3]) }]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lineStarts = file.getLineStarts();
  return (line, column) => {
    const offset = (lineStarts[line - 1] ?? Infinity) + column - 1;
    return calls.filter(([from, to]) => from <= offset && offset <= to).map(([, , at]) => at);
  };
}

