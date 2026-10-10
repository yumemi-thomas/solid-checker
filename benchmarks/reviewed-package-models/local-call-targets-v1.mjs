// Exact local call targets. An own object member needs a stable receiver.
import { ts } from './lower.mjs';
export function unwrapLocal(node) {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
}
export class LocalCallTargets {
  constructor(program, source) {
    this.source = source; this.checker = program.getTypeChecker(); this.helpers = new Map(); this.helpersByDeclaration = new Map(); this.references = new Map(); this.calls = [];
    const collect = node => {
      let name, fn;
      if (ts.isFunctionDeclaration(node) && node.name) { name = node.name; fn = node; }
      else if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
        const value = unwrapLocal(node.initializer);
        if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) { name = node.name; fn = value; }
      } else if (ts.isMethodDeclaration(node) && ts.isObjectLiteralExpression(node.parent) && !ts.isComputedPropertyName(node.name)) { name = node.name; fn = node; }
      else if (ts.isPropertyAssignment(node) && ts.isObjectLiteralExpression(node.parent) && !ts.isComputedPropertyName(node.name)) {
        const value = unwrapLocal(node.initializer);
        if (ts.isArrowFunction(value) || ts.isFunctionExpression(value)) { name = node.name; fn = value; }
      }
      if (fn && !fn.asteriskToken && !fn.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword) &&
        fn.parameters.every(parameter => ts.isIdentifier(parameter.name) && !parameter.initializer && !parameter.dotDotDotToken)) {
        const body = fn.body, returned = body && (ts.isBlock(body)
          ? body.statements.length === 1 && ts.isReturnStatement(body.statements[0]) && body.statements[0].expression : body), symbol = this.checker.getSymbolAtLocation(name);
        if (returned && symbol?.declarations?.length === 1) {
          const helper = { name, fn, returned, expression: unwrapLocal(returned) };
          this.helpers.set(symbol, helper); this.helpersByDeclaration.set(symbol.declarations[0], helper);
        }
      }
      if (ts.isCallExpression(node)) this.calls.push(node);
      if (ts.isIdentifier(node) && !ts.isTypeNode(node.parent)) {
        const symbol = ts.isShorthandPropertyAssignment(node.parent) ? this.checker.getShorthandAssignmentValueSymbol(node.parent) : this.checker.getSymbolAtLocation(node);
        if (symbol) { if (!this.references.has(symbol)) this.references.set(symbol, []); this.references.get(symbol).push(node); }
      }
      if (!ts.isTypeNode(node) && !ts.isImportDeclaration(node)) ts.forEachChild(node, collect);
    }; collect(source);
  }
  helperFor(symbol) {
    return symbol?.declarations?.length === 1 ? this.helpers.get(symbol) ?? this.helpersByDeclaration.get(symbol.declarations[0]) : null;
  }
  constantKey(node, seen = new Set()) {
    node = unwrapLocal(node);
    if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node) || ts.isNumericLiteral(node)) return node.text;
    if (!ts.isIdentifier(node)) return null;
    const symbol = this.checker.getSymbolAtLocation(node); if (!symbol || seen.has(symbol) || symbol.declarations?.length !== 1) return null;
    seen.add(symbol); const declaration = symbol.declarations[0];
    return ts.isVariableDeclaration(declaration) && declaration.getSourceFile() === this.source && declaration.parent.flags & ts.NodeFlags.Const && declaration.initializer
      ? this.constantKey(declaration.initializer, seen) : null;
  }
  receiver(expression, seen = new Set()) {
    expression = unwrapLocal(expression); if (!expression || !ts.isIdentifier(expression)) return null;
    const symbol = this.checker.getSymbolAtLocation(expression); if (!symbol || seen.has(symbol) || symbol.declarations?.length !== 1) return null;
    seen.add(symbol); const declaration = symbol.declarations[0];
    if (!ts.isVariableDeclaration(declaration) || declaration.getSourceFile() !== this.source || !(declaration.parent.flags & ts.NodeFlags.Const) || !declaration.initializer) return null;
    const value = unwrapLocal(declaration.initializer);
    if (ts.isObjectLiteralExpression(value)) return { object: value, declarations: [declaration] };
    const inner = this.receiver(value, seen); return inner && { ...inner, declarations: [...inner.declarations, declaration] };
  }
  stableReceiver(receiver) {
    // Accessors, spreads and receiver-dependent methods can hide replacement.
    if (receiver.object.properties.some(property => ts.isSpreadAssignment(property) || ts.isGetAccessor(property) || ts.isSetAccessor(property) || ts.isComputedPropertyName(property.name))) return false;
    let usesThis = false;
    const scan = node => { if (node.kind === ts.SyntaxKind.ThisKeyword) usesThis = true; ts.forEachChild(node, scan); }; scan(receiver.object);
    if (usesThis) return false;
    const visited = new Set();
    const stable = declaration => {
      const symbol = this.checker.getSymbolAtLocation(declaration.name); if (!symbol) return false;
      if (visited.has(symbol)) return true; visited.add(symbol);
      return (this.references.get(symbol) ?? []).every(reference => {
        if (reference === declaration.name) return true;
        let value = reference;
        while (value.parent && unwrapLocal(value.parent) === unwrapLocal(value)) value = value.parent;
        const parent = value.parent;
        if (ts.isVariableDeclaration(parent) && parent.initializer === value && ts.isIdentifier(parent.name) && parent.parent.flags & ts.NodeFlags.Const) return stable(parent);
        if (!(ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) || parent.expression !== value || parent.questionDotToken) return false;
        if (ts.isElementAccessExpression(parent) && this.constantKey(parent.argumentExpression) === null) return false;
        let member = parent;
        while (member.parent && unwrapLocal(member.parent) === unwrapLocal(member)) member = member.parent;
        const use = member.parent;
        return ts.isCallExpression(use) && use.expression === member && !use.questionDotToken ||
          ts.isVariableDeclaration(use) && use.initializer === member && ts.isIdentifier(use.name) && !!(use.parent.flags & ts.NodeFlags.Const);
      });
    };
    return receiver.declarations.every(stable);
  }
  target(expression, seen = new Set(), aliases = [], members = []) {
    expression = unwrapLocal(expression); if (!expression) return null;
    if (ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression)) {
      if (expression.questionDotToken) return null;
      const receiver = this.receiver(expression.expression), key = ts.isPropertyAccessExpression(expression) ? expression.name.text : this.constantKey(expression.argumentExpression);
      if (!receiver || key === null || !this.stableReceiver(receiver)) return null;
      const symbol = ts.isPropertyAccessExpression(expression) ? this.checker.getSymbolAtLocation(expression.name)
        : this.checker.getPropertyOfType(this.checker.getTypeAtLocation(expression.expression), key);
      if (symbol?.declarations?.length !== 1 || symbol.declarations[0].parent !== receiver.object || seen.has(symbol)) return null;
      const declaration = symbol.declarations[0], edge = { start: expression.getStart(this.source), end: expression.end,
        receiverStart: receiver.declarations[0].name.getStart(this.source), propertyStart: declaration.getStart(this.source), propertyEnd: declaration.end };
      seen.add(symbol);
      const helper = this.helperFor(symbol);
      if (helper) return { helper, aliases, members: [...members, edge] };
      if (ts.isShorthandPropertyAssignment(declaration)) {
        const valueSymbol = this.checker.getShorthandAssignmentValueSymbol(declaration);
        if (!valueSymbol || seen.has(valueSymbol)) return null;
        const valueHelper = this.helperFor(valueSymbol);
        if (valueHelper) return { helper: valueHelper, aliases, members: [...members, edge] };
        return this.target(declaration.name, seen, aliases, [...members, edge]);
      }
      return ts.isPropertyAssignment(declaration) ? this.target(declaration.initializer, seen, aliases, [...members, edge]) : null;
    }
    if (!ts.isIdentifier(expression)) return null;
    const symbol = ts.isShorthandPropertyAssignment(expression.parent) ? this.checker.getShorthandAssignmentValueSymbol(expression.parent) : this.checker.getSymbolAtLocation(expression);
    if (!symbol || seen.has(symbol)) return null; seen.add(symbol);
    const helper = this.helperFor(symbol);
    if (helper) return { helper, aliases, members };
    if (symbol.declarations?.length !== 1) return null;
    const declaration = symbol.declarations[0];
    if (!ts.isVariableDeclaration(declaration) || declaration.getSourceFile() !== this.source || !(declaration.parent.flags & ts.NodeFlags.Const) ||
      !ts.isIdentifier(declaration.name) || !declaration.initializer) return null;
    return this.target(declaration.initializer, seen, [...aliases, { start: declaration.name.getStart(this.source), end: declaration.name.end }], members);
  }
  chain(resolved) {
    const outer = [], seen = new Set();
    while (resolved) {
      const helper = resolved.helper; if (seen.has(helper)) return null; seen.add(helper);
      outer.push({ ...helper, aliases: resolved.aliases, members: resolved.members });
      if (ts.isCallExpression(helper.expression)) {
        if (helper.expression.questionDotToken) return null;
        resolved = this.target(helper.expression.expression); continue;
      }
      if (!(ts.isPropertyAccessExpression(helper.expression) || ts.isElementAccessExpression(helper.expression))) return null;
      let unsupported = false;
      const scan = node => { if (ts.isCallExpression(node) || ts.isNewExpression(node) || ts.isFunctionLike(node) || ts.isAwaitExpression(node)) unsupported = true; ts.forEachChild(node, scan); }; scan(helper.expression);
      return unsupported ? null : outer.reverse();
    }
    return null;
  }
}
