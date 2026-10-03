// Nested own object literals share the same mutation/escape boundary.
import { ts } from './lower.mjs';
import { LocalCallTargets as BaseTargets, unwrapLocal } from './local-call-targets-v1.mjs';
export class LocalCallTargets extends BaseTargets {
  ownMember(member, object) {
    if (member.questionDotToken) return null;
    const key = ts.isPropertyAccessExpression(member) ? member.name.text : this.constantKey(member.argumentExpression);
    if (key === null) return null;
    const symbol = ts.isPropertyAccessExpression(member) ? this.checker.getSymbolAtLocation(member.name)
      : this.checker.getPropertyOfType(this.checker.getTypeAtLocation(member.expression), key);
    return symbol?.declarations?.length === 1 && symbol.declarations[0].parent === object ? symbol.declarations[0] : null;
  }
  receiver(expression, seen = new Set()) {
    expression = unwrapLocal(expression); if (!expression) return null;
    if (ts.isPropertyAccessExpression(expression) || ts.isElementAccessExpression(expression)) {
      const parent = this.receiver(expression.expression, seen); if (!parent) return null;
      const declaration = this.ownMember(expression, parent.object), value = declaration && ts.isPropertyAssignment(declaration) && unwrapLocal(declaration.initializer);
      return value && ts.isObjectLiteralExpression(value) ? { ...parent, object: value } : null;
    }
    if (!ts.isIdentifier(expression)) return null;
    const symbol = this.checker.getSymbolAtLocation(expression);
    if (!symbol || seen.has(symbol) || symbol.declarations?.length !== 1) return null;
    seen.add(symbol); const declaration = symbol.declarations[0];
    if (!ts.isVariableDeclaration(declaration) || declaration.getSourceFile() !== this.source || !(declaration.parent.flags & ts.NodeFlags.Const) || !declaration.initializer) return null;
    const value = unwrapLocal(declaration.initializer);
    if (ts.isObjectLiteralExpression(value)) return { object: value, rootObject: value, declarations: [declaration] };
    const inner = this.receiver(value, seen); return inner && { ...inner, declarations: [...inner.declarations, declaration] };
  }
  stableReceiver(receiver) {
    let unsupported = false;
    const scan = node => {
      if (node.kind === ts.SyntaxKind.ThisKeyword || ts.isObjectLiteralExpression(node) && node.properties.some(property =>
        ts.isSpreadAssignment(property) || ts.isGetAccessor(property) || ts.isSetAccessor(property) || ts.isComputedPropertyName(property.name))) unsupported = true;
      ts.forEachChild(node, scan);
    }; scan(receiver.rootObject);
    if (unsupported) return false;
    const visited = new Map();
    const stable = (declaration, object) => {
      const symbol = this.checker.getSymbolAtLocation(declaration.name); if (!symbol) return false;
      if (visited.has(symbol)) return visited.get(symbol) === object; visited.set(symbol, object);
      return (this.references.get(symbol) ?? []).every(reference => {
        if (reference === declaration.name) return true;
        let value = reference, currentObject = object;
        for (;;) {
          while (value.parent && unwrapLocal(value.parent) === unwrapLocal(value)) value = value.parent;
          const parent = value.parent;
          if (ts.isVariableDeclaration(parent) && parent.initializer === value && ts.isIdentifier(parent.name) && parent.parent.flags & ts.NodeFlags.Const) return stable(parent, currentObject);
          if (!(ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) || parent.expression !== value) return false;
          const property = this.ownMember(parent, currentObject); if (!property) return false;
          const child = ts.isPropertyAssignment(property) && unwrapLocal(property.initializer);
          let member = parent;
          while (member.parent && unwrapLocal(member.parent) === unwrapLocal(member)) member = member.parent;
          const use = member.parent;
          if ((ts.isPropertyAccessExpression(use) || ts.isElementAccessExpression(use)) && use.expression === member) {
            if (!child || !ts.isObjectLiteralExpression(child)) return false;
            value = member; currentObject = child; continue;
          }
          if (ts.isCallExpression(use) && use.expression === member && !use.questionDotToken) return true;
          if (ts.isVariableDeclaration(use) && use.initializer === member && ts.isIdentifier(use.name) && use.parent.flags & ts.NodeFlags.Const)
            return child && ts.isObjectLiteralExpression(child) ? stable(use, child) : true;
          return false;
        }
      });
    };
    return stable(receiver.declarations[0], receiver.rootObject);
  }
}
