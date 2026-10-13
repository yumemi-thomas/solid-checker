// Positive, byte-bound source observations for immediate class methods.
// A tracking-sensitive call is advisory evidence, never a certified defect.
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { SourceExtractor, runtimeEntry } from './source-extractor.mjs';
import { ts } from './lower.mjs';
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const unalias = (checker, symbol) => symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol;
const plainName = node => node && !ts.isComputedPropertyName(node) ? node.text : null;

export class ClassFootprints {
  constructor(host = 'browser', maxDepth = 12) {
    this.host = host; this.maxDepth = maxDepth; this.source = new SourceExtractor(host); this.cache = new Map();
  }
  local(module, symbol) {
    const imported = module.imports.get(symbol);
    if (imported && !imported.namespace) {
      if (['solid-js', '@solidjs/signals'].includes(imported.specifier)) return { native: imported.name, module };
      try { return this.exported(runtimeEntry(module.path, imported.specifier, this.host), imported.name); } catch { return null; }
    }
    const declaration = symbol?.valueDeclaration;
    if (declaration && (ts.isFunctionDeclaration(declaration) || ts.isClassDeclaration(declaration))) return { node: declaration, module };
    const initializer = module.globals.get(symbol) ?? declaration?.initializer;
    if (initializer && (ts.isClassExpression(initializer) || ts.isArrowFunction(initializer) || ts.isFunctionExpression(initializer))) return { node: initializer, module };
    return null;
  }
  exported(path, name, seen = new Set()) {
    const key = path + ':' + name; if (seen.has(key) || seen.size >= this.maxDepth) return null; seen.add(key);
    const module = this.source.module(path), entry = module.exports.get(name);
    if (entry?.local) {
      const symbol = ts.isExportSpecifier(entry.local.parent) ? module.checker.getExportSpecifierLocalTargetSymbol(entry.local.parent) : module.checker.getSymbolAtLocation(entry.local);
      return this.local(module, symbol);
    }
    if (entry?.specifier) return this.exported(runtimeEntry(path, entry.specifier, this.host), entry.name, seen);
    const candidates = [];
    for (const entry of module.exports.values()) if (entry.star) {
      try { const target = this.exported(runtimeEntry(path, entry.star, this.host), name, new Set(seen)); if (target) candidates.push(target); } catch {}
    }
    return candidates.length === 1 ? candidates[0] : null;
  }
  method(owner, name) {
    const candidates = owner.node.members.filter(member => plainName(member.name) === name && (ts.isMethodDeclaration(member) || ts.isGetAccessorDeclaration(member)));
    let replaced = false;
    const scan = node => {
      if (ts.isBinaryExpression(node) && node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && node.operatorToken.kind <= ts.SyntaxKind.LastAssignment &&
        ts.isPropertyAccessExpression(node.left) && node.left.expression.kind === ts.SyntaxKind.ThisKeyword && node.left.name.text === name) replaced = true;
      ts.forEachChild(node, scan);
    };
    scan(owner.node); if (replaced) return null;
    return candidates.length === 1 ? { node: candidates[0], module: owner.module, owner } : null;
  }
  target(expression, context) {
    const { module, owner } = context, checker = module.checker; expression = unwrap(expression);
    if (ts.isIdentifier(expression)) return this.local(module, checker.getSymbolAtLocation(expression));
    if (!ts.isPropertyAccessExpression(expression) || expression.questionDotToken) return null;
    if (expression.expression.kind === ts.SyntaxKind.ThisKeyword && owner) return this.method(owner, expression.name.text);
    const receiver = expression.expression;
    if (!owner || !ts.isPropertyAccessExpression(receiver) || receiver.expression.kind !== ts.SyntaxKind.ThisKeyword || !ts.isPrivateIdentifier(receiver.name)) return null;
    // The receiver is this class's exact private field, initialized with new C.
    // Public fields, reassigned private fields and arbitrary member dispatch stay open.
    const fieldSymbol = checker.getSymbolAtLocation(receiver.name);
    if (!fieldSymbol) return null;
    const field = owner.node.members.find(member => ts.isPropertyDeclaration(member) && checker.getSymbolAtLocation(member.name) === fieldSymbol);
    if (!field || !field.initializer || !ts.isNewExpression(unwrap(field.initializer))) return null;
    let writes = false;
    const scan = node => {
      if (ts.isBinaryExpression(node) && node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && node.operatorToken.kind <= ts.SyntaxKind.LastAssignment && ts.isPropertyAccessExpression(node.left) && checker.getSymbolAtLocation(node.left.name) === fieldSymbol) writes = true;
      ts.forEachChild(node, scan);
    };
    scan(owner.node); if (writes) return null;
    const target = this.target(unwrap(field.initializer).expression, context);
    return target?.node && (ts.isClassExpression(target.node) || ts.isClassDeclaration(target.node)) ? this.method(target, expression.name.text) : null;
  }
  footprint(context, seen = new Set()) {
    if (!context?.node?.body || seen.has(context.node) || seen.size >= this.maxDepth) return { premises: [], gaps: ['unresolved or recursive body'] };
    if (context.node.asteriskToken || context.node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword)) return { premises: [], gaps: ['deferred generator or async body'] };
    const nextSeen = new Set([...seen, context.node]), premises = [], gaps = [];
    const scan = node => {
      if (node !== context.node.body && ts.isFunctionLike(node)) return;
      if (ts.isCallExpression(node) && !node.questionDotToken && !node.arguments.some(ts.isSpreadElement)) {
        const target = this.target(node.expression, context);
        if (target?.native === 'getObserver') {
          const position = context.module.source.getLineAndCharacterOfPosition(node.getStart(context.module.source));
          premises.push({ path: context.module.path, sourceSha256: hash(context.module.source.text), start: node.getStart(context.module.source), line: position.line + 1,
            operation: 'exact getObserver call in a non-deferred source body' });
        } else if (target?.node) {
          const child = this.footprint(target, nextSeen);
          premises.push(...child.premises); gaps.push(...child.gaps);
        }
      }
      ts.forEachChild(node, scan);
    };
    scan(context.node.body);
    return { premises: [...new Map(premises.map(p => [JSON.stringify(p), p])).values()], gaps: [...new Set(gaps)] };
  }
  extract(project, packageName, name) {
    const key = project + ':' + packageName + ':' + name; if (this.cache.has(key)) return this.cache.get(key);
    const started = performance.now(), result = { package: packageName, export: name, authority: false, methods: {}, getters: {}, gaps: [], refused: null };
    try {
      const root = packageRoot(project, packageName); result.version = read(join(root, 'package.json')).version; result.pins = closurePins(root);
      for (const runtime of nativeRuntimeRoots(project)) if (read(join(runtime, 'package.json')).version !== '2.0.0-rc.9') throw new Error('runtime outside rc.9 vocabulary');
      for (const pin of result.pins.filter(pin => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(pin.package))) if (pin.version !== '2.0.0-rc.9') throw new Error('nested runtime outside rc.9 vocabulary');
      const target = this.exported(runtimeEntry(join(project, 'App.mjs'), packageName, this.host), name);
      if (!target || !(ts.isClassDeclaration(target.node) || ts.isClassExpression(target.node))) throw new Error('export is not an exact source class');
      for (const member of target.node.members) {
        const name = plainName(member.name); if (!name || !(ts.isMethodDeclaration(member) || ts.isGetAccessorDeclaration(member))) continue;
        const observation = this.footprint({ node: member, module: target.module, owner: target });
        if (observation.premises.length) (ts.isGetAccessorDeclaration(member) ? result.getters : result.methods)[name] = observation;
        if (observation.gaps.length) result.gaps.push({ member: name, gaps: observation.gaps });
      }
      result.sources = [...this.source.modules.values()].map(module => ({ path: module.path, sha256: hash(module.source.text) }));
      if (JSON.stringify(closurePins(root)) !== JSON.stringify(result.pins)) throw new Error('source closure changed');
    } catch (error) { result.refused = error.message; }
    result.durationMs = performance.now() - started; this.cache.set(key, result); return result;
  }
}

export function classSnapshotFlows(program, source, engine = new ClassFootprints()) {
  const checker = program.getTypeChecker(), imports = new Map(), namespaces = new Map(), instances = new Map(), candidates = [], refused = [];
  for (const statement of source.statements) if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier) && !statement.importClause?.isTypeOnly) {
    const clause = statement.importClause?.namedBindings;
    if (clause && ts.isNamedImports(clause)) {
      for (const item of clause.elements) if (!item.isTypeOnly)
        imports.set(checker.getSymbolAtLocation(item.name), { package: statement.moduleSpecifier.text, name: (item.propertyName ?? item.name).text });
    } else if (clause && ts.isNamespaceImport(clause)) namespaces.set(checker.getSymbolAtLocation(clause.name), statement.moduleSpecifier.text);
  }
  function imported(expression) {
    expression = unwrap(expression);
    if (ts.isIdentifier(expression)) return imports.get(checker.getSymbolAtLocation(expression));
    if (ts.isPropertyAccessExpression(expression) && ts.isIdentifier(expression.expression)) {
      const packageName = namespaces.get(checker.getSymbolAtLocation(expression.expression));
      if (packageName) return { package: packageName, name: expression.name.text };
    }
    return null;
  }
  function declaration(expression, model) {
    const target = unalias(checker, checker.getSymbolAtLocation(ts.isPropertyAccessExpression(expression) ? expression.name : expression));
    const root = packageRoot(dirname(source.fileName), model.package);
    return target?.declarations?.length && target.declarations.every(declaration => {
      const path = relative(root, declaration.getSourceFile().fileName); return path && !path.startsWith('..') && !path.startsWith('/');
    }) ? target.declarations : null;
  }
  function visitInstances(node) {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && ts.isNewExpression(unwrap(node.initializer)) && node.parent.flags & ts.NodeFlags.Const) {
      const creation = unwrap(node.initializer), target = imported(creation.expression);
      if (target?.package && !['solid-js', '@solidjs/web'].includes(target.package)) {
        const model = engine.extract(dirname(source.fileName), target.package, target.name);
        const declarations = model.refused ? null : declaration(creation.expression, model);
        if (declarations) instances.set(checker.getSymbolAtLocation(node.name), { model, node, declarations });
      }
    }
    ts.forEachChild(node, visitInstances);
  }
  visitInstances(source);
  // Reject any instance reference that might escape, replace a method, or
  // dispatch through a computed target. Const alone does not freeze an object.
  function scanUses(node) {
    if (ts.isTypeNode(node)) return;
    if (ts.isIdentifier(node)) {
      const instance = instances.get(checker.getSymbolAtLocation(node));
      if (instance && node !== instance.node.name) {
        const parent = node.parent;
        const member = ts.isPropertyAccessExpression(parent) && parent.expression === node && !parent.questionDotToken;
        const write = member && (ts.isBinaryExpression(parent.parent) && parent.parent.left === parent && parent.parent.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && parent.parent.operatorToken.kind <= ts.SyntaxKind.LastAssignment);
        const memberDeclarations = member && checker.getSymbolAtLocation(parent.name)?.declarations;
        const exactMember = memberDeclarations?.length && memberDeclarations.every(declaration => instance.declarations.includes(declaration.parent));
        let callTarget = parent;
        while (callTarget.parent && (ts.isParenthesizedExpression(callTarget.parent) || ts.isAsExpression(callTarget.parent) || ts.isNonNullExpression(callTarget.parent) || ts.isSatisfiesExpression(callTarget.parent))) callTarget = callTarget.parent;
        const immediateCall = member && ts.isCallExpression(callTarget.parent) && callTarget.parent.expression === callTarget;
        const callableMember = member && checker.getTypeAtLocation(parent).getCallSignatures().length;
        if (!member || write || !exactMember || callableMember && !immediateCall) instance.open = true;
      }
    }
    ts.forEachChild(node, scanUses);
  }
  scanUses(source);
  const jsxUse = symbol => {
    const spans = []; const scan = (node, inJsx = false) => {
      if (inJsx && ts.isFunctionLike(node)) return;
      if (ts.isJsxExpression(node)) inJsx = true;
      if (inJsx && ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol) spans.push({ start: node.getStart(source), end: node.end });
      ts.forEachChild(node, child => scan(child, inJsx));
    }; scan(source); return spans;
  };
  function snapshot(node) {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const value = unwrap(node.initializer), expression = ts.isCallExpression(value) ? unwrap(value.expression) : value;
      if (ts.isPropertyAccessExpression(expression) && ts.isIdentifier(expression.expression) && !expression.questionDotToken && !(ts.isCallExpression(value) && (value.questionDotToken || value.arguments.some(ts.isSpreadElement)))) {
        const instance = instances.get(checker.getSymbolAtLocation(expression.expression));
        if (instance) {
          const observation = (ts.isCallExpression(value) ? instance.model.methods : instance.model.getters)[expression.name.text];
          const use = jsxUse(checker.getSymbolAtLocation(node.name));
          // Only straight function-body setup is admitted. Callbacks, events,
          // JSX, conditional branches and top-level module reads remain open.
          const block = node.parent.parent.parent, owner = block?.parent;
          const setup = ts.isVariableStatement(node.parent.parent) && ts.isBlock(block) && ts.isFunctionLike(owner) && owner.body === block;
          if (observation?.premises.length && use.length && setup) {
            if (instance.open) refused.push({ start: value.getStart(source), reason: 'instance escapes or has mutable/computed dispatch' });
            else candidates.push({ code: 'SOURCE_CLASS_SNAPSHOT_FLOW', severity: 'info', certification: false, basis: 'source positive footprint; intended liveness undeclared',
              package: instance.model.package, export: instance.model.export, member: expression.name.text, start: value.getStart(source), end: value.end, jsxUses: use, premises: observation.premises });
          }
        }
      }
    }
    ts.forEachChild(node, snapshot);
  }
  snapshot(source);
  return { candidates, refused, models: [...instances.values()].map(instance => instance.model) };
}
