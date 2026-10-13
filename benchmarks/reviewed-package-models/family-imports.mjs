// Enumerate exact named imports and direct namespace calls used by a consumer.
// Computed dispatch and shadowed namespace names provide no package premise.
import { ts } from './lower.mjs';
export function familyRequests(program, source, packages) {
  const checker = program.getTypeChecker(), allowed = new Set(packages), requests = new Map(), namespaces = new Map(), gaps = [];
  const target = symbol => symbol && symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
  const add = (name, exported) => { if (!requests.has(name)) requests.set(name, new Set()); requests.get(name).add(exported); };
  for (const statement of source.statements) {
    if (!ts.isImportDeclaration(statement) || !allowed.has(statement.moduleSpecifier.text)) continue;
    const binding = statement.importClause?.namedBindings, name = statement.moduleSpecifier.text;
    if (binding && ts.isNamedImports(binding)) for (const element of binding.elements) {
      if (!element.isTypeOnly) add(name, (element.propertyName ?? element.name).text);
    }
    if (binding && ts.isNamespaceImport(binding)) {
      const symbol = checker.getSymbolAtLocation(binding.name), module = target(symbol);
      if (module) namespaces.set(symbol, { name, exports: checker.getExportsOfModule(module) });
    }
  }
  function visit(node) {
    if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && ts.isIdentifier(node.expression.expression)) {
      const namespace = namespaces.get(checker.getSymbolAtLocation(node.expression.expression));
      const member = target(checker.getSymbolAtLocation(node.expression.name));
      const exported = namespace?.exports.filter(symbol => target(symbol) === member);
      if (exported?.length === 1 && !node.questionDotToken && !node.expression.questionDotToken) add(namespace.name, exported[0].name);
    }
    if (ts.isCallExpression(node) && ts.isElementAccessExpression(node.expression) && ts.isIdentifier(node.expression.expression) &&
      namespaces.has(checker.getSymbolAtLocation(node.expression.expression))) gaps.push({ start: node.getStart(source), reason: 'computed namespace dispatch remains unresolved' });
    ts.forEachChild(node, visit);
  }
  visit(source);
  return { requests: [...requests].map(([packageName, exports]) => ({ package: packageName, exports: [...exports].sort() })), gaps };
}
