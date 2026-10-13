// Bounded development instrumentation of resolved core imports. This records
// a guarded runtime path; it never declares a package contract or a defect.
import { readFileSync } from "node:fs";
import { dirname } from "node:path";
import { hash, nativeRuntimeRoots, read } from "./catalog.mjs";
import { ts } from "./lower.mjs";
const unwrap = node => { while (ts.isParenthesizedExpression(node)) node = node.expression; return node; };
export function instrumentGuards(text, path) {
  const program = ts.createProgram([path], { allowJs: true, noResolve: true, noLib: true, target: ts.ScriptTarget.ESNext }, {
    ...ts.createCompilerHost({ allowJs: true }), getSourceFile(file) { return file === path ? ts.createSourceFile(path, text, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS) : undefined; },
  });
  const source = program.getSourceFile(path), checker = program.getTypeChecker(), imports = new Map(), edits = [], observations = [];
  if (!source || source.parseDiagnostics.length) return null;
  for (const node of source.statements) if (ts.isImportDeclaration(node) && ['solid-js', '@solidjs/signals'].includes(node.moduleSpecifier.text)) {
    const names = node.importClause?.namedBindings;
    if (names && ts.isNamedImports(names)) for (const name of names.elements) imports.set(checker.getSymbolAtLocation(name.name), (name.propertyName ?? name.name).text);
  }
  function callIs(node, name) { node = unwrap(node); return ts.isCallExpression(node) && ts.isIdentifier(node.expression) && imports.get(checker.getSymbolAtLocation(node.expression)) === name; }
  function hasCall(node, name, skipNested = false) {
    let found = false;
    function scan(child) { if (child !== node && skipNested && ts.isFunctionLike(child)) return; if (callIs(child, name)) found = true; if (!found) ts.forEachChild(child, scan); }
    scan(node); return found;
  }
  function nearestFunction(node) { for (let parent = node.parent; parent; parent = parent.parent) if (ts.isFunctionLike(parent)) return parent; return null; }
  function observe(call, kind) {
    if (edits.some(edit => edit.start === call.getStart(source))) return;
    const position = source.getLineAndCharacterOfPosition(call.getStart(source));
    const premise = { kind, path, line: position.line + 1, column: position.character + 1, sourceSha256: hash(text), start: call.getStart(source), end: call.end };
    const original = call.getText(source);
    edits.push({ start: call.getStart(source), end: call.end,
      text: `((__guardValue) => { if (!__guardValue) globalThis.__solidGuardTrace?.record(${JSON.stringify(premise)}); return __guardValue; })(${original})` });
    observations.push(premise);
  }
  function visit(node) {
    if (ts.isIfStatement(node)) {
      const condition = unwrap(node.expression);
      if (callIs(condition, 'getOwner') && hasCall(node.thenStatement, 'onCleanup', true)) observe(condition, 'automatic-cleanup-skipped');
      if (ts.isPrefixUnaryExpression(condition) && condition.operator === ts.SyntaxKind.ExclamationToken && callIs(condition.operand, 'getObserver')) {
        const fn = nearestFunction(node), body = fn?.body;
        // A concrete guard leading to a return, in the same function as an
        // exact core source creation. Do not infer arbitrary helper semantics.
        const immediateReturn = ts.isReturnStatement(node.thenStatement) || ts.isBlock(node.thenStatement) && node.thenStatement.statements.length === 1 && ts.isReturnStatement(node.thenStatement.statements[0]);
        if (immediateReturn && body && hasCall(body, 'createSignal', true)) observe(unwrap(condition.operand), 'tracking-skipped');
      }
    }
    if (ts.isConditionalExpression(node) && callIs(node.condition, 'getOwner') && hasCall(node.whenTrue, 'onCleanup', true)) observe(unwrap(node.condition), 'automatic-cleanup-skipped');
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken && callIs(node.left, 'getOwner') && hasCall(node.right, 'onCleanup', true)) observe(unwrap(node.left), 'automatic-cleanup-skipped');
    ts.forEachChild(node, visit);
  }
  visit(source);
  if (!edits.length) return null;
  let code = text;
  for (const edit of edits.sort((a, b) => b.start - a.start)) code = code.slice(0, edit.start) + edit.text + code.slice(edit.end);
  return { code, observations };
}
export function guardTracePlugin() {
  const transformed = [], refused = [];
  return { name: 'observed-package-guards', enforce: 'pre', transformed, refused,
    transform(code, id) {
      const path = id.split('?')[0];
      if (!path.includes('/node_modules/') || !path.endsWith('.js') || path.includes('/@solidjs/signals/') || path.includes('/solid-js/')) return null;
      try {
        for (const root of nativeRuntimeRoots(dirname(path))) if (read(root + '/package.json').version !== '2.0.0-rc.9') throw new Error('Unsupported runtime for guard instrumentation');
        // The transformation must describe the installed file, not another
        // plugin's already-rewritten source or a synthetic optimizer bundle.
        if (readFileSync(path, 'utf8') !== code) return null;
        const result = instrumentGuards(code, path);
        if (!result) return null;
        transformed.push(...result.observations);
        return { code: result.code, map: null };
      } catch (error) { refused.push({ path, reason: error.message }); return null; }
    } };
}
