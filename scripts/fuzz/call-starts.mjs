// Review 4c (reviewer's artifact): V8 places a member call's frame at the property (`map` in `providers.map(...)`), and a `new` or tagged
// call's at its callee, while a checker row starts at the call expression (or its callee's object). A store or props read or write is
// a trap call on the same terms: V8 places it at the property (`x` in `st.x`, `[0]`'s key) and a `delete` at its keyword, while the row
// starts at the access. `callStarts(source)` maps a frame position that lies in the callee of a call, `new`, tagged template or JSX tag,
// in the property of a member access, or on a `delete`, to that expression's start and its object's start, read from the program text.
// A frame anywhere else keeps only its own position, so each frame is still exact positions, never a range: a position in a call's
// arguments is not the call. `starts.args` are a call's argument starts, for a top frame only (`frameStarts`), and `starts.mount` says a
// frame is at a mount (a component tag, a `render(` callee), by position: a one-line program's other JSX mounts nothing here.
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
// Loaded on first use, never on import: audit.mjs reaches this module through focus.mjs, and its `pick` and `report` jobs run with no
// install, parse no program, and failed on the import alone (run 42517832347).
let loaded = null;
const typescript = () => (loaded ??= createRequire(join(resolve(here, "../../packages/cli"), "package.json"))("typescript"));
const cache = new Map();
/** Solid's own functions that read the value they are handed: unlike a primitive that raises its own codes, their arguments stay handed (R23). */
const VALUE_READERS = new Set(["deep", "snapshot", "merge", "omit"]);

/**
 * `(frame, depth) => starts`: a frame's starts at its depth in the stack (0 is the top authored frame). A top frame in a call's callee has
 * no authored frame inside that call: native or dependency code did the read or write, on what the call was handed, and V8 gives no
 * other position for it. An argument is such a start, marked `handed`: it counts for a withholding on a code about a handed value and
 * only there (codes.mjs `handedCodes`, `withheldAtStack`). Never for a Solid primitive or setter callee, which raises its own codes;
 * never deeper, where an argument is not the read (R5, R20).
 */
/**
 * The statement holding a position, as `contains(line, column)`, or null: the outermost one starting on its line, so the statements of an
 * IIFE on that line are one site. A one-line program holds several statements on a line; a finding in a sibling statement is about
 * something else.
 */
export function statementAt(source, line, column, fileName = "App.tsx") {
  const ts = typescript();
  const text = String(source ?? "");
  const file = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, fileName.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const offsetOf = (l, c) => (file.getLineStarts()[l - 1] ?? Infinity) + c - 1;
  const at = offsetOf(line, column);
  let best = null;
  const visit = (node) => {
    if (ts.isStatement(node) && !ts.isBlock(node) && node.getStart(file) <= at && at < node.getEnd() && (!best || node.getEnd() - node.getStart(file) < best.getEnd() - best.getStart(file))) best = node;
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lineOf = (node) => file.getLineAndCharacterOfPosition(node.getStart(file)).line;
  for (let up = best?.parent; up; up = up.parent) if (ts.isStatement(up) && !ts.isBlock(up) && lineOf(up) === lineOf(best)) best = up;
  return best && { contains: (l, c) => best.getStart(file) <= offsetOf(l, c) && offsetOf(l, c) < best.getEnd() };
}

/**
 * The lines that say whether the function holding `line` runs: its own, and every reference to its name (`const hf = ...; [].forEach(hf)`).
 */
export function carrierLines(source, line, fileName = "App.tsx") {
  const ts = typescript();
  const text = String(source ?? "");
  const file = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, fileName.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const lineOf = (offset) => file.getLineAndCharacterOfPosition(offset).line + 1;
  const names = new Set();
  const visit = (node) => {
    if ((ts.isFunctionDeclaration(node) || ts.isArrowFunction(node) || ts.isFunctionExpression(node)) && lineOf(node.getStart(file)) <= line && line <= lineOf(node.getEnd() - 1)) {
      const named = ts.isFunctionDeclaration(node) ? node.name : ts.isVariableDeclaration(node.parent) && ts.isIdentifier(node.parent.name) ? node.parent.name : null;
      if (named) names.add(named.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lines = new Set([line]);
  const refs = (node) => { if (ts.isIdentifier(node) && names.has(node.text)) lines.add(lineOf(node.getStart(file))); ts.forEachChild(node, refs); };
  refs(file);
  return [...lines];
}

export function frameStarts(source, fileName = "App.tsx") {
  const starts = callStarts(source, fileName);
  return (frame, depth) => [...starts(frame.line, frame.column), ...(depth === 0 ? starts.args(frame.line, frame.column).map((place) => ({ ...place, handed: true })) : [])];
}

/** `(line, column) => [{ line, column }]` (1-based): the starts a frame at that position stands for, its own position first. */
export function callStarts(source, fileName = "App.tsx") {
  const ts = typescript();
  const text = String(source ?? "");
  const key = `${fileName}\u0000${text}`;
  if (cache.has(key)) return cache.get(key);
  const file = ts.createSourceFile(fileName, text, ts.ScriptTarget.Latest, true, fileName.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const position = (offset) => { const at = file.getLineAndCharacterOfPosition(offset); return { line: at.line + 1, column: at.character + 1 }; };
  const unwrap = (expression) => {
    let inner = expression;
    while (ts.isParenthesizedExpression(inner) || ts.isAsExpression(inner) || ts.isSatisfiesExpression(inner) || ts.isNonNullExpression(inner)) inner = inner.expression;
    return inner;
  };
  /** The object a member chain hangs off, through parentheses, casts and `!`. */
  const rootOf = (expression) => {
    let object = unwrap(expression);
    while (ts.isPropertyAccessExpression(object) || ts.isElementAccessExpression(object)) object = unwrap(object.expression);
    return object;
  };
  // [frame range start, end, expression start, object start] for every call-like expression (its callee), member access (its property),
  // `delete` (its keyword) and `for...of` (its binding, where V8 places the iterator's step; the row is at the iterable).
  const calls = [];
  // [callee start, callee end, argument starts, whether no argument is known to be a store write's target] for every call and `new`:
  // `list.push(x)` writes `list`, not `x`; a dependency's body is unread, so nothing says what it writes; a static `Object.assign(st, x)`
  // writes its first argument.
  const handed = [];
  // [start, end] of everything that mounts: a component tag, a `render(` or `hydrate(` callee.
  const mounts = [];
  // The names Solid's own code stands behind: what a Solid module exports here, and a tuple a primitive returned (`[c, setC]`). Such a
  // callee raises its own codes (a setter's write, `untrack`'s run): its arguments are not what it read.
  const solid = new Set();
  const dependency = new Set();
  for (const statement of file.statements) {
    if (!ts.isImportDeclaration(statement) || statement.moduleSpecifier.text.startsWith(".")) continue;
    const names = /^(?:solid-js|@solidjs\/)/.test(statement.moduleSpecifier.text) ? solid : dependency;
    const clause = statement.importClause;
    if (clause?.name) names.add(clause.name.text);
    if (clause?.namedBindings) (ts.isNamespaceImport(clause.namedBindings) ? [clause.namedBindings] : clause.namedBindings.elements).forEach((element) => { if (!VALUE_READERS.has(element.propertyName?.text ?? element.name.text)) names.add(element.name.text); });
  }
  const nameOf = (expression) => (ts.isIdentifier(unwrap(expression)) ? unwrap(expression).text : null);
  const collect = (node) => {
    if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && node.initializer && ts.isCallExpression(unwrap(node.initializer)) && solid.has(nameOf(rootOf(unwrap(node.initializer).expression)))) {
      for (const element of node.name.elements) if (ts.isBindingElement(element) && ts.isIdentifier(element.name)) solid.add(element.name.text);
    }
    ts.forEachChild(node, collect);
  };
  collect(file);
  const noTarget = (callee) => dependency.has(nameOf(rootOf(callee))) || ((ts.isPropertyAccessExpression(unwrap(callee)) || ts.isElementAccessExpression(unwrap(callee))) && !/^(?:Object|Reflect)$/.test(nameOf(rootOf(callee)) ?? ""));
  const access = (node, from, to) => calls.push([from, to, node.getStart(file), rootOf(node).getStart(file)]);
  /** A store read on a call's result has no source mapping past the call's `)`: V8's frame comes back there. */
  const onResult = (node) => {
    const inner = unwrap(node.expression);
    if (ts.isCallExpression(inner) || ts.isNewExpression(inner)) access(node, inner.getEnd() - 1, inner.getEnd());
  };
  const visit = (node) => {
    let callee = null;
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) callee = node.expression;
    else if (ts.isTaggedTemplateExpression(node)) callee = node.tag;
    else if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) callee = node.tagName;
    if (callee) {
      calls.push([callee.getStart(file), callee.getEnd(), node.getStart(file), rootOf(callee).getStart(file)]);
      if (node.arguments?.length && !solid.has(nameOf(rootOf(callee)))) handed.push([callee.getStart(file), callee.getEnd(), node.arguments.map((argument) => argument.getStart(file)), noTarget(callee)]);
      if (ts.isCallExpression(node) && /^(?:render|hydrate)$/.test(callee.getText(file))) mounts.push([callee.getStart(file), callee.getEnd()]);
      if (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) {
        const tag = callee.getText(file);
        if (/^[A-Z_$]/.test(tag) || tag.includes(".")) mounts.push([node.getStart(file), callee.getEnd()]);
      }
    }
    // A tagged call's frame is on its template, the row on the tag.
    if (ts.isTaggedTemplateExpression(node)) calls.push([node.template.getStart(file), node.template.getEnd(), node.getStart(file), rootOf(node.tag).getStart(file)]);
    if (ts.isPropertyAccessExpression(node)) { access(node, node.name.getStart(file), node.name.getEnd()); onResult(node); }
    else if (ts.isElementAccessExpression(node)) {
      access(node, node.expression.getEnd(), node.getEnd());
      onResult(node);
      // A member key: V8 places the keyed load at the key's property, which stands for the key's read and for this access.
      if (ts.isPropertyAccessExpression(node.argumentExpression)) access(node, node.argumentExpression.name.getStart(file), node.argumentExpression.name.getEnd());
    } else if (ts.isDeleteExpression(node)) access(node.expression, node.getStart(file), node.expression.getStart(file));
    else if (ts.isForOfStatement(node)) access(node.expression, node.initializer.getStart(file), node.initializer.getEnd());
    // The compiler keeps no mapping inside an attribute's closure: V8's frame in `run={() => n()}` comes back on `run`, the row on the call.
    // Only with exactly one thing inside that can read: of two, the frame does not say which (R24, R25). A callee's own member is its call.
    else if (ts.isJsxAttribute(node) && node.initializer) {
      const inside = [];
      const reads = (child) => (ts.isCallExpression(child) || ts.isNewExpression(child) || ts.isTaggedTemplateExpression(child) || ts.isSpreadElement(child) || ts.isSpreadAssignment(child)
        || (ts.isBinaryExpression(child) && child.operatorToken.kind === ts.SyntaxKind.InKeyword)
        || ((ts.isPropertyAccessExpression(child) || ts.isElementAccessExpression(child)) && !((ts.isCallExpression(child.parent) || ts.isNewExpression(child.parent)) && child.parent.expression === child)));
      const inner = (child) => { if (reads(child)) inside.push(child); ts.forEachChild(child, inner); };
      inner(node.initializer);
      if (inside.length === 1) calls.push([node.name.getStart(file), node.name.getEnd(), inside[0].getStart(file), rootOf(inside[0].expression ?? inside[0]).getStart(file)]);
    }
    // `in` reads its right operand: V8's frame is at the binary's start, the row on the operand.
    else if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.InKeyword) calls.push([node.getStart(file), node.right.getStart(file), node.right.getStart(file), rootOf(node.right).getStart(file)]);
    // A spread steps its operand: V8's frame is on the operand, the row on the `...`.
    else if (ts.isSpreadElement(node) || ts.isSpreadAssignment(node)) calls.push([node.expression.getStart(file), node.expression.getEnd(), node.getStart(file), rootOf(node.expression).getStart(file)]);
    // A template coerces its substitutions natively: V8's frame is the template's, the row on the substitution.
    else if (ts.isTemplateExpression(node)) for (const span of node.templateSpans) calls.push([node.getStart(file), node.head.getEnd(), span.expression.getStart(file), rootOf(span.expression).getStart(file)]);
    // A destructuring reads its properties: V8's frame is on the binding, the row on the pattern.
    else if (ts.isBindingElement(node)) { let pattern = node.parent; while (ts.isBindingElement(pattern.parent)) pattern = pattern.parent.parent; calls.push([node.getStart(file), node.getEnd(), pattern.getStart(file), pattern.getStart(file)]); }
    // The same as an assignment: `({ v } = st)`.
    else if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken && (ts.isObjectLiteralExpression(unwrap(node.left)) || ts.isArrayLiteralExpression(unwrap(node.left)))) {
      const left = unwrap(node.left);
      for (const part of ts.isObjectLiteralExpression(left) ? left.properties : left.elements) calls.push([part.getStart(file), part.getEnd(), left.getStart(file), left.getStart(file)]);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  const lineStarts = file.getLineStarts();
  const offsetOf = (line, column) => (lineStarts[line - 1] ?? Infinity) + column - 1;
  const size = (range) => range[1] - range[0];
  /** The smallest ranges that hold `offset`. */
  const smallest = (ranges, offset) => {
    const holding = ranges.filter((range) => range[0] <= offset && offset < range[1]);
    const least = Math.min(...holding.map(size));
    return holding.filter((range) => size(range) === least);
  };
  const distinct = (places) => places.filter((at, index) => places.findIndex((other) => other.line === at.line && other.column === at.column) === index);
  const starts = (line, column) => distinct([{ line, column }, ...smallest(calls, offsetOf(line, column)).flatMap((range) => [position(range[2]), position(range[3])])]);
  // The arguments of the call whose callee holds the position.
  starts.args = (line, column) => smallest(handed, offsetOf(line, column)).flatMap((range) => range[2].map((offset, index) => ({ ...position(offset), index, noTarget: range[3] })));
  starts.mount = (line, column) => { const offset = offsetOf(line, column); return mounts.some((range) => range[0] <= offset && offset < range[1]); };
  cache.set(key, starts);
  if (cache.size > 64) cache.delete(cache.keys().next().value);
  return starts;
}
