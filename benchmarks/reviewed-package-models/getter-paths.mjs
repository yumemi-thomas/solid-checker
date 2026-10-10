// Finite positive source paths. This extension never certifies package behavior.
// Runtime/declaration identity and intent remain separate proof requirements.
import { readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { closurePins, hash, nativeRuntimeRoots, packageRoot, read } from './catalog.mjs';
import { SourceExtractor, literal, objectValue, runtimeEntry, unknownValue } from './source-extractor.mjs';
import { ts } from './lower.mjs';
const alternatives = value => value.kind === 'union' ? value.values.flatMap(alternatives) : [value];
const unwrap = node => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node))) node = node.expression;
  return node;
};
const clone = state => ({ ...state, env: new Map(state.env), operations: [...state.operations], unknowns: [...state.unknowns], blockers: [...state.blockers], reads: [...(state.reads ?? [])] });
const empty = () => ({ env: new Map(), owned: false, observed: false, operations: [], unknowns: [], blockers: [], reads: [] });
function finiteObject(value) {
  const values = alternatives(value);
  if (!values.length || values.some(value => value.kind !== 'object')) return null;
  const keys = Object.keys(values[0].fields);
  if (!values.every(value => JSON.stringify(Object.keys(value.fields)) === JSON.stringify(keys))) return null;
  return objectValue(Object.fromEntries(keys.map(key => [key, values.every(value => value.fields[key] === values[0].fields[key]) ? values[0].fields[key] : unknownValue('joined field value')])));
}

export class GetterPaths extends SourceExtractor {
  constructor(host = 'browser') { super(host); this.intrinsics = new Map(); }
  intrinsic(node, module, member) {
    const expression = unwrap(node.expression);
    if (!ts.isPropertyAccessExpression(expression) || expression.name.text !== member || !ts.isIdentifier(expression.expression)) return false;
    if (!this.intrinsics.has(module.path)) {
      const program = ts.createProgram([module.path], { allowJs: true, noResolve: true, target: ts.ScriptTarget.ESNext });
      const source = program.getSourceFile(module.path), checker = program.getTypeChecker(), sites = new Map();
      const visit = node => {
        if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(unwrap(node.expression))) {
          const expression = unwrap(node.expression), target = checker.getSymbolAtLocation(expression.name), receiver = checker.getSymbolAtLocation(expression.expression);
          // Resolve the real global Object and its lib.es5 declarations. A
          // shadowed Object, a guessed member or a local same-name helper fails.
          const globalObject = receiver?.declarations?.some(declaration => declaration.getSourceFile().fileName.endsWith('/lib.es5.d.ts') && ts.isVariableDeclaration(declaration) && declaration.name.getText() === 'Object');
          const exactMember = target?.declarations?.some(declaration => declaration.getSourceFile().fileName.endsWith('/lib.es5.d.ts') && ts.isInterfaceDeclaration(declaration.parent) && declaration.parent.name.text === 'ObjectConstructor');
          if (globalObject && exactMember) sites.set(node.getStart(source), expression.name.text);
        }
        ts.forEachChild(node, visit);
      };
      if (source && !source.parseDiagnostics.length) visit(source); this.intrinsics.set(module.path, sites);
    }
    return this.intrinsics.get(module.path).get(node.getStart(module.source)) === member;
  }
  merge(states, outer) {
    super.merge(states, outer);
    // Record positive observations from the interpreted paths, not absence.
    outer.reads = [...new Map([...(outer.reads ?? []), ...states.flatMap(state => state.reads ?? [])].map(read => [JSON.stringify(read), read])).values()];
  }
  invoke(value, args, state, depth) {
    if (value.kind === 'accessor') {
      state.reads.push({ producer: value.evidence, read: { path: state.callModule?.path, start: state.callNode?.getStart(state.callModule.source) } });
      return value.shape ?? unknownValue('accessor output');
    }
    if (value.kind === 'each') {
      if (value.items.length > 32 || args[0]?.kind !== 'function') { state.blockers.push('unresolved finite forEach'); return unknownValue('forEach'); }
      for (let index = 0; index < value.items.length; index++) this.invoke(args[0], [value.items[index], literal(index)], state, depth + 1);
      return literal(undefined);
    }
    if (value.kind === 'native' && value.name === 'getObserver') return literal(state.observed ? true : null);
    if (value.kind === 'native' && value.name === 'createMemo') {
      const evidence = { path: state.callModule.path, start: state.callNode.getStart(state.callModule.source), native: `${value.package}.${value.name}` };
      // Compute interpretation determines only finite object keys. Unknown
      // values, caller captures and side effects do not become runtime facts.
      const inner = clone(state); inner.observed = true;
      const shape = args[0]?.kind === 'function' ? this.invoke(args[0], [], inner, depth + 1) : unknownValue('unknown memo compute');
      state.unknowns.push(...inner.unknowns);
      if (inner.blockers.length) state.unknowns.push('memo output key interpretation has blockers');
      return { kind: 'accessor', evidence, shape: inner.blockers.length ? unknownValue('memo output shape is open') : shape };
    }
    return super.invoke(value, args, state, depth);
  }
  expression(node, module, state, depth) {
    node = unwrap(node); if (!node) return literal(undefined);
    if (depth > this.maxDepth || ++this.steps > this.maxSteps) { state.blockers.push('getter expression budget'); return unknownValue('budget'); }
    if (ts.isObjectLiteralExpression(node)) {
      const fields = Object.create(null);
      for (const property of node.properties) {
        if (property.name?.text === '__proto__') { state.blockers.push('object prototype initializer'); return unknownValue('prototype'); }
        if (ts.isSpreadAssignment(property)) {
          const value = finiteObject(this.expression(property.expression, module, state, depth + 1));
          if (!value || Object.values(value.fields).some(value => value.kind === 'getter')) { state.blockers.push('unresolved or getter-bearing object spread'); return unknownValue('object spread'); }
          Object.assign(fields, value.fields);
        } else if (ts.isPropertyAssignment(property) && !ts.isComputedPropertyName(property.name)) fields[property.name.text] = this.expression(property.initializer, module, state, depth + 1);
        else if (ts.isShorthandPropertyAssignment(property)) fields[property.name.text] = this.expression(property.name, module, state, depth + 1);
        else if (ts.isGetAccessorDeclaration(property) && !ts.isComputedPropertyName(property.name)) fields[property.name.text] = { kind: 'getter', callback: { kind: 'function', node: property, module, env: new Map(state.env) } };
        else { state.blockers.push('unresolved object field'); return unknownValue('object field'); }
      }
      return objectValue(fields);
    }
    if (ts.isBinaryExpression(node) && node.operatorToken.kind === ts.SyntaxKind.EqualsToken && (ts.isElementAccessExpression(node.left) || ts.isPropertyAccessExpression(node.left))) {
      const receiver = this.expression(node.left.expression, module, state, depth + 1);
      const key = ts.isPropertyAccessExpression(node.left) ? literal(node.left.name.text) : this.expression(node.left.argumentExpression, module, state, depth + 1);
      const value = this.expression(node.right, module, state, depth + 1);
      if (receiver.kind !== 'object' || key.kind !== 'literal' || typeof key.value !== 'string') { state.blockers.push('unresolved member assignment'); return unknownValue('assignment'); }
      receiver.fields[key.value] = value; return value;
    }
    if (ts.isDeleteExpression(node) && ts.isElementAccessExpression(node.expression)) {
      const receiver = this.expression(node.expression.expression, module, state, depth + 1), key = this.expression(node.expression.argumentExpression, module, state, depth + 1);
      if (receiver.kind !== 'object' || key.kind !== 'literal') { state.blockers.push('unresolved delete'); return unknownValue('delete'); }
      delete receiver.fields[key.value]; return literal(true);
    }
    if (ts.isCallExpression(node) && !node.questionDotToken && !node.arguments.some(ts.isSpreadElement)) {
      if (this.intrinsic(node, module, 'keys') && node.arguments.length === 1) {
        const value = finiteObject(this.expression(node.arguments[0], module, state, depth + 1));
        if (value) return { kind: 'array', items: Object.keys(value.fields).map(literal) };
        state.blockers.push('unresolved Object.keys input'); return unknownValue('keys');
      }
      if (this.intrinsic(node, module, 'defineProperty') && node.arguments.length === 3) {
        const [receiver, key, descriptor] = node.arguments.map(argument => this.expression(argument, module, state, depth + 1));
        if (receiver.kind !== 'object' || key.kind !== 'literal' || typeof key.value !== 'string' || descriptor.kind !== 'object' || descriptor.fields.get?.kind !== 'function' || descriptor.fields.set) {
          state.blockers.push('unresolved getter installation'); return unknownValue('defineProperty');
        }
        receiver.fields[key.value] = { kind: 'getter', callback: descriptor.fields.get }; return receiver;
      }
    }
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const receiver = this.expression(node.expression, module, state, depth + 1);
      const key = ts.isPropertyAccessExpression(node) ? literal(node.name.text) : this.expression(node.argumentExpression, module, state, depth + 1);
      if (receiver.kind === 'array' && key.kind === 'literal' && key.value === 'forEach') return { kind: 'each', items: receiver.items };
      if (receiver.kind === 'object' && key.kind === 'literal' && receiver.fields[key.value]?.kind === 'getter') {
        state.callNode = node; state.callModule = module;
        return this.invoke(receiver.fields[key.value].callback, [], state, depth + 1);
      }
      // Avoid evaluating the receiver twice, including calls with mutation.
      if (receiver.kind === 'object' && key.kind === 'literal') return receiver.fields[key.value] ?? literal(undefined);
      if (receiver.kind === 'array' && key.kind === 'literal') return key.value === 'slice' ? { kind: 'array-slice', items: receiver.items } : receiver.items[key.value] ?? unknownValue('array member');
      if (receiver.kind === 'namespace' && ts.isPropertyAccessExpression(node)) return this.imported(receiver.module, { specifier: receiver.specifier, name: key.value }, state, depth);
      return unknownValue('unresolved member');
    }
    return super.expression(node, module, state, depth);
  }
  statements(nodes, module, states, depth) {
    if (depth > this.maxDepth) return states.map(state => ({ ...state, blockers: [...state.blockers, 'getter control depth budget'] }));
    for (const node of nodes) {
      if (!ts.isForInStatement(node)) { states = super.statements([node], module, states, depth); continue; }
      const next = [];
      for (const state of states) {
        if (state.returned) { next.push(state); continue; }
        const object = finiteObject(this.expression(node.expression, module, state, depth + 1));
        const declaration = ts.isVariableDeclarationList(node.initializer) && node.initializer.declarations.length === 1 ? node.initializer.declarations[0] : null;
        if (!object || !declaration || !ts.isIdentifier(declaration.name) || Object.keys(object.fields).length > 32) { state.blockers.push('unresolved finite for-in'); next.push(state); continue; }
        let paths = [state];
        for (const key of Object.keys(object.fields)) {
          for (const path of paths) this.bind(declaration.name, literal(key), module, path);
          paths = this.statements(ts.isBlock(node.statement) ? node.statement.statements : [node.statement], module, paths, depth + 1);
        }
        next.push(...paths);
      }
      states = next;
    }
    return states;
  }
  argument(node, file) {
    const module = this.module(file), start = node.getStart(), end = node.end; let matched = null;
    const visit = own => { if (own.getStart(module.source) === start && own.end === end && own.kind === node.kind) matched = own; if (!matched) ts.forEachChild(own, visit); };
    visit(module.source); if (!matched) return unknownValue('consumer source span unresolved');
    const state = empty(), value = this.expression(matched, module, state, 0);
    return state.blockers.length ? unknownValue('consumer argument has unresolved execution') : value;
  }
  extractGetters(path, name, args = []) {
    this.steps = 0; const module = this.module(path), state = empty(), target = this.exportValue(module, name, state, 0);
    const result = { name, fields: [], gaps: [], returnKinds: [], certification: false };
    if (target.kind !== 'function') return { ...result, gaps: ['export is not an exact function'] };
    const returned = this.invoke(target, args, state, 0), values = alternatives(returned); result.returnKinds = values.map(value => value.kind);
    const containers = values.every(value => value.kind === 'object') ? [{ path: [], values }] :
      values.every(value => value.kind === 'array') ? Array.from({ length: Math.min(...values.map(value => value.items.length)) }, (_, index) => ({ path: [index], values: values.flatMap(value => alternatives(value.items[index])) })) : [];
    for (const container of containers) if (finiteObject({ kind: 'union', values: container.values })) for (const key of Object.keys(container.values[0].fields)) {
      const getters = container.values.map(value => value.fields[key]); if (getters.some(value => value.kind !== 'getter')) continue;
      const observations = getters.map(value => {
        const inner = clone(state); inner.observed = true; inner.reads = []; inner.blockers = [];
        this.invoke(value.callback, [], inner, 0); result.gaps.push(...inner.blockers, ...inner.unknowns); return inner;
      });
      if (!state.blockers.length && observations.every(inner => !inner.blockers.length && inner.reads.length)) result.fields.push({ path: [...container.path, key], getterOnly: true,
        alternatives: observations.length, reads: [...new Map(observations.flatMap(inner => inner.reads).map(read => [JSON.stringify(read), read])).values()] });
    }
    result.gaps = [...new Set([...result.gaps, ...state.blockers, ...state.unknowns])]; return result;
  }
  installed(project, packageName, name, args) {
    const started = performance.now(), root = packageRoot(project, packageName), pins = closurePins(root);
    for (const path of nativeRuntimeRoots(project)) if (read(join(path, 'package.json')).version !== '2.0.0-rc.9') throw new Error('runtime outside rc.9');
    for (const pin of pins.filter(pin => ['solid-js', '@solidjs/signals', '@solidjs/web'].includes(pin.package))) if (pin.version !== '2.0.0-rc.9') throw new Error('nested runtime outside rc.9');
    const result = this.extractGetters(runtimeEntry(join(project, 'App.mjs'), packageName, this.host), name, args);
    if (JSON.stringify(closurePins(root)) !== JSON.stringify(pins)) throw new Error('source closure changed');
    return { ...result, package: packageName, version: read(join(root, 'package.json')).version, pins, durationMs: performance.now() - started,
      sources: [...this.modules.values()].map(module => ({ path: module.path, sha256: hash(readFileSync(module.path)) })) };
  }
}

export function getterConsumerFlows(program, source) {
  const checker = program.getTypeChecker(), imports = new Map(), namespaces = new Map(), containers = new Map(), models = [], open = [], notes = [];
  for (const statement of source.statements) if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier) && !statement.importClause?.isTypeOnly) {
    const names = statement.importClause?.namedBindings, packageName = statement.moduleSpecifier.text;
    if (names && ts.isNamedImports(names)) for (const item of names.elements) imports.set(checker.getSymbolAtLocation(item.name), { package: packageName, name: (item.propertyName ?? item.name).text });
    else if (names && ts.isNamespaceImport(names)) namespaces.set(checker.getSymbolAtLocation(names.name), packageName);
  }
  function target(node) {
    node = unwrap(node);
    if (ts.isIdentifier(node)) return imports.get(checker.getSymbolAtLocation(node));
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && !node.questionDotToken) {
      const packageName = namespaces.get(checker.getSymbolAtLocation(node.expression)); if (packageName) return { package: packageName, name: node.name.text };
    }
    return null;
  }
  function populate(node) {
    if (ts.isVariableDeclaration(node) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const call = unwrap(node.initializer), exported = ts.isCallExpression(call) && !call.questionDotToken && !call.arguments.some(ts.isSpreadElement) ? target(call.expression) : null;
      if (exported && !['solid-js', '@solidjs/signals', '@solidjs/web'].includes(exported.package)) {
        try {
          const symbol = checker.getSymbolAtLocation(ts.isPropertyAccessExpression(unwrap(call.expression)) ? unwrap(call.expression).name : unwrap(call.expression));
          const original = symbol?.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol, root = packageRoot(dirname(source.fileName), exported.package);
          if (!original?.declarations?.length || !original.declarations.every(declaration => { const path = relative(root, declaration.getSourceFile().fileName); return path && !path.startsWith('..') && !path.startsWith('/'); })) throw new Error('export declaration is not in resolved package');
          const engine = new GetterPaths(), args = call.arguments.map(argument => engine.argument(argument, source.fileName));
          const model = engine.installed(dirname(source.fileName), exported.package, exported.name, args); models.push(model);
          const bind = (name, prefix) => {
            if (!ts.isIdentifier(name)) return;
            const fields = model.fields.filter(field => field.path.length === prefix.length + 1 && prefix.every((part, index) => field.path[index] === part));
            if (fields.length) containers.set(checker.getSymbolAtLocation(name), { model, name, fields });
          };
          if (ts.isIdentifier(node.name)) bind(node.name, []);
          else if (ts.isArrayBindingPattern(node.name)) node.name.elements.forEach((item, index) => { if (ts.isBindingElement(item) && !item.dotDotDotToken) bind(item.name, [index]); });
        } catch (error) { open.push({ start: call.getStart(source), reason: error.message }); }
      }
    }
    ts.forEachChild(node, populate);
  }
  populate(source);
  const uses = new Map();
  function inspect(node) {
    if (ts.isTypeNode(node)) return;
    if (ts.isIdentifier(node)) {
      const symbol = checker.getSymbolAtLocation(node), container = containers.get(symbol);
      if (container && node !== container.name) {
        const parent = node.parent, direct = ts.isPropertyAccessExpression(parent) && parent.expression === node && !parent.questionDotToken;
        const assignment = direct && ts.isBinaryExpression(parent.parent) && parent.parent.left === parent && parent.parent.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && parent.parent.operatorToken.kind <= ts.SyntaxKind.LastAssignment;
        if (!direct || assignment) container.escaped = true;
        if (!direct || ts.isCallExpression(parent.parent) && parent.parent.expression === parent) container.escapedOther = true;
        if (assignment) container.writes = [...(container.writes ?? []), { node: parent.parent, key: parent.name.text }];
      }
      // Only direct JSX value use, excluding event and other deferred callbacks.
      let parent = node.parent, jsx = false;
      while (parent) { if (ts.isFunctionLike(parent)) break; if (ts.isJsxExpression(parent)) { jsx = true; break; } parent = parent.parent; }
      if (jsx) uses.set(symbol, [...(uses.get(symbol) ?? []), { start: node.getStart(source), end: node.end }]);
    }
    ts.forEachChild(node, inspect);
  }
  inspect(source);
  function snapshots(node) {
    if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name) && node.initializer && node.parent.flags & ts.NodeFlags.Const) {
      const value = unwrap(node.initializer), container = ts.isPropertyAccessExpression(value) && ts.isIdentifier(value.expression) && !value.questionDotToken ? containers.get(checker.getSymbolAtLocation(value.expression)) : null;
      const field = container?.fields.find(field => field.path.at(-1) === value.name.text), jsxUses = uses.get(checker.getSymbolAtLocation(node.name));
      const block = node.parent.parent.parent, owner = block?.parent;
      let callback = owner;
      while (callback?.parent && (ts.isParenthesizedExpression(callback.parent) || ts.isAsExpression(callback.parent) || ts.isNonNullExpression(callback.parent) || ts.isSatisfiesExpression(callback.parent))) callback = callback.parent;
      const caller = callback && ts.isCallExpression(callback.parent) ? callback.parent : null, core = caller && target(caller.expression);
      const tracked = core && ['solid-js', '@solidjs/signals'].includes(core.package) && ['createMemo', 'createEffect', 'createTrackedEffect', 'untrack'].includes(core.name);
      if (field && jsxUses?.length && ts.isBlock(block) && ts.isFunctionLike(owner) && !tracked) {
        if (container.escaped) open.push({ start: value.getStart(source), reason: 'returned object escapes or is written' });
        else notes.push({ code: 'SOURCE_GETTER_SNAPSHOT_FLOW', severity: 'info', certification: false, basis: 'finite source getter path; intent and full closure undeclared',
          package: container.model.package, export: container.model.name, field: field.path, start: value.getStart(source), end: value.end, jsxUses, reads: field.reads });
      }
    }
    ts.forEachChild(node, snapshots);
  }
  snapshots(source);
  for (const container of containers.values()) for (const write of container.writes ?? []) {
    const field = container.fields.find(field => field.path.at(-1) === write.key);
    if (field && !container.escapedOther) notes.push({ code: 'SOURCE_GETTER_WRITE_CANDIDATE', severity: 'info', certification: false,
      basis: 'source installation has no setter; subsequent descriptor changes remain open',
      package: container.model.package, export: container.model.name, field: field.path, start: write.node.getStart(source), end: write.node.end, reads: field.reads });
  }
  return { notes, models, open };
}
