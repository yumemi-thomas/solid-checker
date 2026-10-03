// Instrument exact onSettled registrations in installed dependency JavaScript.
// No package-specific callback contracts or guessed dispatch are admitted.
import { readFileSync } from "node:fs";
import { dirname } from "node:path";
import { hash, nativeRuntimeRoots, read } from "./catalog.mjs";
import { ts } from "./lower.mjs";
export function instrumentOrigins(text, path) {
  const program = ts.createProgram([path], { allowJs: true, noResolve: true, noLib: true, target: ts.ScriptTarget.ESNext }, {
    ...ts.createCompilerHost({ allowJs: true }), getSourceFile(file) { return file === path ? ts.createSourceFile(path, text, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS) : undefined; },
  });
  const source = program.getSourceFile(path), checker = program.getTypeChecker(), imports = new Set(), edits = [], registrations = [];
  if (!source || source.parseDiagnostics.length) return null;
  for (const node of source.statements) if (ts.isImportDeclaration(node) && ['solid-js', '@solidjs/signals'].includes(node.moduleSpecifier.text)) {
    const names = node.importClause?.namedBindings;
    if (names && ts.isNamedImports(names)) for (const name of names.elements)
      if ((name.propertyName ?? name.name).text === 'onSettled') imports.add(checker.getSymbolAtLocation(name.name));
  }
  function visit(node) {
    if (ts.isCallExpression(node) && !node.questionDotToken && ts.isIdentifier(node.expression) && imports.has(checker.getSymbolAtLocation(node.expression)) && node.arguments.length === 1 && !ts.isSpreadElement(node.arguments[0])) {
      const argument = node.arguments[0], position = source.getLineAndCharacterOfPosition(node.getStart(source));
      const premise = { operation: 'onSettled', path, line: position.line + 1, column: position.character + 1,
        sourceSha256: hash(text), start: node.getStart(source), end: node.end };
      registrations.push(premise);
      edits.push({ start: argument.getStart(source), end: argument.end,
        text: `((__callback) => globalThis.__packageOrigins ? globalThis.__packageOrigins.wrap(__callback, ${JSON.stringify(premise)}) : __callback)(${argument.getText(source)})` });
      // Avoid overlapping nested callback edits in this bounded prototype.
      return;
    }
    ts.forEachChild(node, visit);
  }
  visit(source); if (!edits.length) return null;
  let code = text; for (const edit of edits.sort((a, b) => b.start - a.start)) code = code.slice(0, edit.start) + edit.text + code.slice(edit.end);
  return { code, registrations };
}
export function instrumentSettledFire(text, path) {
  // An optional trace hook in the exact published core, scoped to the one
  // synchronous scheduler branch. Runtime behavior and diagnostics stay intact.
  const program = ts.createProgram([path], { allowJs: true, noResolve: true, noLib: true, target: ts.ScriptTarget.ESNext }, {
    ...ts.createCompilerHost({ allowJs: true }), getSourceFile(file) { return file === path ? ts.createSourceFile(path, text, ts.ScriptTarget.ESNext, true, ts.ScriptKind.JS) : undefined; },
  });
  const source = program.getSourceFile(path), checker = program.getTypeChecker();
  if (!source || source.parseDiagnostics.length) return null;
  const declaration = source.statements.find(node => ts.isFunctionDeclaration(node) && node.name?.text === 'onSettled');
  if (!declaration || declaration.parameters.length !== 1 || !ts.isIdentifier(declaration.parameters[0].name)) return null;
  const parameter = declaration.parameters[0].name.text, parameterSymbol = checker.getSymbolAtLocation(declaration.parameters[0].name), candidates = [];
  function visit(node) {
    if (ts.isFunctionExpression(node) && node.name?.text === 'fire' && node.parameters.length === 0) {
      let invokesParameter = false, rejectsCleanup = false;
      function scan(child) {
        if (child !== node.body && ts.isFunctionLike(child)) return;
        if (ts.isVariableDeclaration(child) && ts.isIdentifier(child.name) && child.name.text === 'cleanup' && child.initializer && ts.isCallExpression(child.initializer) && ts.isIdentifier(child.initializer.expression) && checker.getSymbolAtLocation(child.initializer.expression) === parameterSymbol && child.initializer.arguments.length === 0) invokesParameter = true;
        if (ts.isPropertyAssignment(child) && ts.isIdentifier(child.name) && child.name.text === 'code' && ts.isStringLiteral(child.initializer) && child.initializer.text === 'SETTLED_CLEANUP_UNOWNED') rejectsCleanup = true;
        ts.forEachChild(child, scan);
      }
      scan(node.body); if (invokesParameter && rejectsCleanup) candidates.push(node);
    }
    ts.forEachChild(node, visit);
  }
  visit(declaration); if (candidates.length !== 1) return null;
  const fire = candidates[0], body = fire.body.getText(source);
  const position = source.getLineAndCharacterOfPosition(fire.body.getStart(source));
  const premise = { operation: 'onSettled-runtime-fire', path, line: position.line + 1, column: position.character + 1,
    sourceSha256: hash(text), start: fire.body.getStart(source), end: fire.body.end };
  const replacement = `{ return globalThis.__packageOrigins ? globalThis.__packageOrigins.invokeSettled(${parameter}, () => ${body}) : (() => ${body})(); }`;
  return { code: text.slice(0, premise.start) + replacement + text.slice(premise.end), registrations: [premise] };
}
export function originTracePlugin() {
  const transformed = [], refused = [];
  return { name: 'observed-package-origins', enforce: 'pre', transformed, refused, transform(code, id) {
    const path = id.split('?')[0];
    const settledCore = path.endsWith('/@solidjs/signals/dist/dev.js');
    if (!path.includes('/node_modules/') || !path.endsWith('.js') || !settledCore && (path.includes('/@solidjs/signals/') || path.includes('/solid-js/'))) return null;
    try {
      for (const root of nativeRuntimeRoots(dirname(path))) if (read(root + '/package.json').version !== '2.0.0-rc.9') throw new Error('Unsupported runtime for origin instrumentation');
      if (readFileSync(path, 'utf8') !== code) return null;
      const result = settledCore ? instrumentSettledFire(code, path) : instrumentOrigins(code, path); if (!result) return null;
      transformed.push(...result.registrations); return { code: result.code, map: null };
    } catch (error) { refused.push({ path, reason: error.message }); return null; }
  } };
}
