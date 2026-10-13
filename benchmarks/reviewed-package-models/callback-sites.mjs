// Lexical identity of the experiment's own signal setter and tagged callback.
// Real published type admission and runtime identity are verified separately.
import { ts } from './lower.mjs';
export function callbackSites(path) {
  const program = ts.createProgram([path], { noResolve: true, noLib: true, target: ts.ScriptTarget.Latest }), source = program.getSourceFile(path), checker = program.getTypeChecker();
  const callbacks = [], setters = new Set(), nativeSignals = new Set();
  for (const statement of source.statements) if (ts.isImportDeclaration(statement) && statement.moduleSpecifier.text === 'solid-js' &&
    statement.importClause?.namedBindings && ts.isNamedImports(statement.importClause.namedBindings)) {
    for (const binding of statement.importClause.namedBindings.elements) if ((binding.propertyName ?? binding.name).text === 'createSignal') nativeSignals.add(checker.getSymbolAtLocation(binding.name));
  }
  function symbols(node) {
    if (ts.isVariableDeclaration(node) && ts.isArrayBindingPattern(node.name) && node.initializer && ts.isCallExpression(node.initializer) &&
      ts.isIdentifier(node.initializer.expression) && nativeSignals.has(checker.getSymbolAtLocation(node.initializer.expression))) {
      const setter = node.name.elements[1]; if (setter && ts.isBindingElement(setter) && ts.isIdentifier(setter.name)) setters.add(checker.getSymbolAtLocation(setter.name));
    }
    ts.forEachChild(node, symbols);
  }
  symbols(source);
  function marker(node) {
    if (!ts.isBlock(node.body)) return null;
    let id = null;
    function find(child) {
      if (child !== node.body && (ts.isArrowFunction(child) || ts.isFunctionExpression(child))) return;
      if (ts.isCallExpression(child) && ts.isPropertyAccessExpression(child.expression) && child.expression.name.text === 'push' &&
        ts.isPropertyAccessExpression(child.expression.expression) && child.expression.expression.name.text === 'samples' && child.arguments[0] && ts.isObjectLiteralExpression(child.arguments[0])) {
        const field = child.arguments[0].properties.find(p => ts.isPropertyAssignment(p) && p.name.text === 'id');
        if (field && ts.isNumericLiteral(field.initializer)) id = Number(field.initializer.text);
      }
      ts.forEachChild(child, find);
    }
    find(node.body); return id;
  }
  function visit(node, callback = null) {
    if (ts.isArrowFunction(node)) { const id = marker(node); callback = id === null ? null : { id, start: node.getStart(), end: node.end, writes: [] }; if (callback) callbacks.push(callback); }
    if (callback && ts.isCallExpression(node) && ts.isIdentifier(node.expression) && setters.has(checker.getSymbolAtLocation(node.expression))) {
      const statement = node.parent;
      callback.writes.push({ start: node.getStart(), end: node.end, statementStart: ts.isExpressionStatement(statement) ? statement.getStart() : node.getStart(), statementEnd: statement.end });
    }
    ts.forEachChild(node, child => visit(child, callback));
  }
  visit(source);
  return { source, callbacks, writes: callbacks.flatMap(c => c.writes.map(w => ({ ...w, id: c.id }))) };
}
export function callbackWriteAt(diagnostic, path, sites) {
  const location = diagnostic.originalLocation; if (!location || location.path !== path || !location.line || !location.column) return null;
  let offset; try { offset = sites.source.getPositionOfLineAndCharacter(location.line - 1, location.column - 1); } catch { return null; }
  return sites.writes.find(write => offset >= write.start && offset < write.end) ?? null;
}
export function tokensWithoutWrites(sites) {
  let text = sites.source.text;
  for (const write of [...sites.writes].sort((a,b) => b.statementStart-a.statementStart)) text = text.slice(0,write.statementStart) + text.slice(write.statementEnd);
  const scanner = ts.createScanner(ts.ScriptTarget.Latest, true, ts.LanguageVariant.Standard, text), tokens = [];
  for (let token = scanner.scan(); token !== ts.SyntaxKind.EndOfFileToken; token = scanner.scan()) tokens.push([token, scanner.getTokenText()]);
  return tokens;
}
