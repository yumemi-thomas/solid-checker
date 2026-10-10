// Bounded experiment: extract positive source premises, never certify a package
// or infer an empty behavior domain from an absence of observations.
import { existsSync, readFileSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { packageRoot, read } from "./catalog.mjs";
import { ts } from "./lower.mjs";

const unknown = reason => ({ kind: "unknown", reason });
export const unknownValue = unknown;
export const callableValue = () => ({ kind: "callable" });
export const literal = value => ({ kind: "literal", value });
export const objectValue = fields => ({ kind: "object", fields });
const choices = value => value.kind === "union" ? value.values.flatMap(choices) : [value];
const union = values => {
  const unique = [...new Set(values.flatMap(choices))];
  return unique.length === 1 ? unique[0] : { kind: "union", values: unique };
};
const truth = value => value.kind === "literal" ? Boolean(value.value) : null;
const fork = state => ({ ...state, env: new Map(state.env), operations: [...state.operations], unknowns: [...state.unknowns], blockers: [...state.blockers] });

function conditionalTarget(value, host) {
  if (typeof value === "string") return value;
  if (!value || Array.isArray(value)) throw new Error("Unsupported runtime export map");
  // Follow export-map ordering, as module resolution does; never choose types
  // or a package's unshipped source condition.
  for (const [condition, target] of Object.entries(value))
    if (["import", host, "development", "solid", "default"].includes(condition)) {
      const selected = conditionalTarget(target, host);
      if (selected) return selected;
    }
  return null;
}

export function runtimeEntry(from, specifier, host) {
  if (specifier.startsWith(".")) {
    const path = resolve(dirname(from), specifier);
    if (!existsSync(path) || ![".js", ".mjs", ".jsx"].includes(extname(path))) throw new Error(`Missing runtime file: ${path}`);
    return path;
  }
  const parts = specifier.split("/"), name = parts.slice(0, specifier.startsWith("@") ? 2 : 1).join("/");
  const root = packageRoot(dirname(from), name), manifest = read(join(root, "package.json"));
  const subpath = parts.slice(name.split("/").length).join("/");
  let map = manifest.exports;
  if (map && typeof map === "object" && Object.keys(map).some(key => key.startsWith("."))) map = map[subpath ? `./${subpath}` : "."];
  if (subpath && !manifest.exports) throw new Error(`Unmapped subpath: ${specifier}`);
  const target = map ? conditionalTarget(map, host) : manifest.module ?? manifest.main;
  if (!target) throw new Error(`Missing runtime export: ${specifier}`);
  const path = resolve(root, target);
  if (!path.startsWith(root + "/") || !existsSync(path)) throw new Error(`Missing or escaped runtime entry: ${specifier}`);
  return path;
}

export class SourceExtractor {
  constructor(host, { maxDepth = 10, maxSteps = 4000, maxPaths = 16 } = {}) {
    this.host = host; this.maxDepth = maxDepth; this.maxSteps = maxSteps; this.maxPaths = maxPaths;
    this.modules = new Map(); this.steps = 0;
  }
  module(path) {
    path = resolve(path);
    if (this.modules.has(path)) return this.modules.get(path);
    const program = ts.createProgram([path], { allowJs: true, noResolve: true, noLib: true, target: ts.ScriptTarget.Latest });
    const source = program.getSourceFile(path), checker = program.getTypeChecker();
    if (!source || source.parseDiagnostics.length) throw new Error(`Unparsed source: ${path}`);
    const module = { path, source, checker, exports: new Map(), imports: new Map(), globals: new Map() };
    this.modules.set(path, module);
    for (const statement of source.statements) {
      if (ts.isImportDeclaration(statement) && ts.isStringLiteral(statement.moduleSpecifier)) {
        const bindings = statement.importClause?.namedBindings;
        if (bindings && ts.isNamedImports(bindings)) for (const item of bindings.elements)
          module.imports.set(checker.getSymbolAtLocation(item.name), { specifier: statement.moduleSpecifier.text, name: (item.propertyName ?? item.name).text });
        else if (bindings && ts.isNamespaceImport(bindings))
          module.imports.set(checker.getSymbolAtLocation(bindings.name), { specifier: statement.moduleSpecifier.text, namespace: true });
      }
      if (ts.isFunctionDeclaration(statement) && statement.name) module.globals.set(checker.getSymbolAtLocation(statement.name), statement);
      if (ts.isVariableStatement(statement)) for (const item of statement.declarationList.declarations)
        if (ts.isIdentifier(item.name)) module.globals.set(checker.getSymbolAtLocation(item.name), item.initializer);
      if (statement.modifiers?.some(item => item.kind === ts.SyntaxKind.ExportKeyword) && statement.name)
        module.exports.set(statement.name.text, { local: statement.name });
      if (ts.isVariableStatement(statement) && statement.modifiers?.some(item => item.kind === ts.SyntaxKind.ExportKeyword))
        for (const item of statement.declarationList.declarations) if (ts.isIdentifier(item.name)) module.exports.set(item.name.text, { local: item.name });
      if (ts.isExportDeclaration(statement) && statement.exportClause && ts.isNamedExports(statement.exportClause))
        for (const item of statement.exportClause.elements) module.exports.set(item.name.text,
          statement.moduleSpecifier ? { specifier: statement.moduleSpecifier.text, name: (item.propertyName ?? item.name).text } : { local: item.propertyName ?? item.name });
      if (ts.isExportDeclaration(statement) && !statement.exportClause && statement.moduleSpecifier)
        module.exports.set(`*${statement.pos}`, { star: statement.moduleSpecifier.text });
    }
    return module;
  }
  exportNames(path, visited = new Set()) {
    if (visited.has(path)) return []; visited.add(path);
    const module = this.module(path), names = [];
    for (const [name, entry] of module.exports) {
      if (!entry.star) names.push(name);
      else try { names.push(...this.exportNames(runtimeEntry(path, entry.star, this.host), visited).filter(item => item !== "default")); } catch {}
    }
    return [...new Set(names)].sort();
  }
  imported(module, entry, state, depth) {
    if (entry.namespace) return { kind: "namespace", module, specifier: entry.specifier };
    if (["solid-js", "@solidjs/signals"].includes(entry.specifier)) return { kind: "native", package: entry.specifier, name: entry.name };
    if (entry.specifier === "@solidjs/web" && entry.name === "isServer") return literal(this.host === "node");
    try { return this.exportValue(this.module(runtimeEntry(module.path, entry.specifier, this.host)), entry.name, state, depth + 1); }
    catch (error) { state.unknowns.push(error.message); return unknown(error.message); }
  }
  exportValue(module, name, state, depth) {
    if (depth > this.maxDepth) { state.blockers.push("export depth budget"); return unknown("depth"); }
    const entry = module.exports.get(name);
    if (entry?.local) return this.expression(entry.local, module, state, depth);
    if (entry?.specifier) return this.imported(module, entry, state, depth);
    const candidates = [];
    for (const item of module.exports.values()) if (item.star) {
      try {
        const next = this.module(runtimeEntry(module.path, item.star, this.host));
        if (this.exportNames(next.path).includes(name)) candidates.push(this.exportValue(next, name, state, depth + 1));
      } catch (error) { state.unknowns.push(error.message); }
    }
    if (candidates.length === 1 || candidates.length > 1 && candidates.every(item => item.node && item.node === candidates[0].node)) return candidates[0];
    if (candidates.length > 1) { state.blockers.push("ambiguous star export"); return unknown("ambiguous export"); }
    return unknown(`unresolved export ${name}`);
  }
  merge(states, outer) {
    // Keep only mandatory operations. A conditional registration remains a
    // candidate gap, even if most branches happen to register cleanup.
    const kinds = ["cleanup", "effect"];
    const operations = kinds.filter(kind => states.every(state => state.operations.some(item => item.kind === kind)))
      .flatMap(kind => states[0].operations.filter(item => item.kind === kind));
    outer.operations = [...new Map([...outer.operations, ...operations].map(item => [JSON.stringify(item), item])).values()];
    outer.unknowns = [...new Set([...outer.unknowns, ...states.flatMap(state => state.unknowns)])];
    outer.blockers = [...new Set([...outer.blockers, ...states.flatMap(state => state.blockers)])];
    // Join writes to captured caller bindings, including conditional writes.
    // Callee-local symbols cannot overwrite a caller symbol with the same name.
    for (const key of outer.env.keys())
      outer.env.set(key, union(states.map(state => state.env.get(key) ?? outer.env.get(key))));
  }
  invoke(value, args, state, depth) {
    if (depth > this.maxDepth) { state.blockers.push("call depth budget"); return unknown("depth"); }
    if (value.kind === "union") {
      const states = value.values.map(() => fork(state));
      const results = value.values.map((item, i) => this.invoke(item, args, states[i], depth + 1));
      this.merge(states, state); return union(results);
    }
    if (value.kind === "array-slice") {
      const start = args[0] ?? literal(0), end = args[1] ?? literal(value.items.length);
      if (args.length > 2 || [start, end].some(item => item.kind !== "literal" || !Number.isInteger(item.value)))
        return unknown("unresolved array slice bounds");
      // The experiment assumes standard intrinsic Array.prototype.slice.
      // Only a previously established array receiver uses this operation.
      return { kind: "array", items: value.items.slice(start.value, end.value) };
    }
    if (value.kind === "native") {
      const evidence = { path: state.callModule.path, start: state.callNode.getStart(state.callModule.source), native: `${value.package}.${value.name}` };
      if (value.name === "DEV") return literal(true);
      if (value.name === "getOwner") return literal(state.owned ? true : null);
      if (value.name === "createSignal") return { kind: "array", items: [{ kind: "accessor", evidence }, { kind: "setter" }] };
      if (value.name === "createMemo") return { kind: "accessor", evidence };
      if (["onCleanup", "createEffect", "createRenderEffect"].includes(value.name)) {
        if (!state.owned) state.operations.push({ kind: value.name === "onCleanup" ? "cleanup" : "effect", ...evidence });
        return unknown("native registration return");
      }
      if (["untrack", "createRoot"].includes(value.name)) {
        const inner = fork(state); inner.owned ||= value.name === "createRoot";
        const result = this.invoke(args[0] ?? unknown("missing callback"), [], inner, depth + 1);
        this.merge([inner], state); return result;
      }
      // Other native APIs have no negative/absence premise. In particular,
      // deferred callbacks are never walked as if they execute in the caller.
      state.unknowns.push(`unmodeled native ${value.name}`); return unknown(value.name);
    }
    if (value.kind !== "function") {
      state.unknowns.push(value.reason ?? `unmodeled call ${value.kind}`);
      for (const arg of args) if (arg.kind === "function") {
        const captured = new Set(arg.env.keys());
        const inspect = node => {
          if (ts.isBinaryExpression(node) && node.operatorToken.kind >= ts.SyntaxKind.FirstAssignment && node.operatorToken.kind <= ts.SyntaxKind.LastAssignment && ts.isIdentifier(node.left) &&
            captured.has(arg.module.checker.getSymbolAtLocation(node.left))) state.blockers.push("escaped mutable closure with unknown timing");
          ts.forEachChild(node, inspect);
        };
        inspect(arg.node.body);
      }
      return unknown("unmodeled call");
    }
    const { node, module } = value, inner = fork(state);
    inner.env = new Map(value.env);
    for (let i = 0; i < node.parameters.length; i++) {
      const parameter = node.parameters[i];
      if (parameter.dotDotDotToken) { inner.blockers.push("rest parameter"); continue; }
      let arg = args[i] ?? literal(undefined);
      if (arg.kind === "literal" && arg.value === undefined && parameter.initializer) arg = this.expression(parameter.initializer, module, inner, depth + 1);
      this.bind(parameter.name, arg, module, inner);
    }
    let endings;
    if (ts.isBlock(node.body)) endings = this.statements(node.body.statements, module, [inner], depth + 1);
    else { inner.returnValue = this.expression(node.body, module, inner, depth + 1); inner.returned = true; endings = [inner]; }
    this.merge(endings, state);
    return union(endings.map(item => item.returned ? item.returnValue : literal(undefined)));
  }
  bind(name, value, module, state) {
    if (ts.isIdentifier(name)) state.env.set(module.checker.getSymbolAtLocation(name), value);
    else if (ts.isArrayBindingPattern(name)) name.elements.forEach((item, i) => {
      if (!ts.isBindingElement(item)) return;
      if (item.dotDotDotToken) { state.blockers.push("rest binding"); return; }
      this.bind(item.name, union(choices(value).map(choice => choice.kind === "array" ? choice.items[i] ?? literal(undefined) : unknown("unresolved tuple"))), module, state);
    });
    else if (ts.isObjectBindingPattern(name)) for (const item of name.elements) {
      if (item.dotDotDotToken) { state.blockers.push("rest binding"); continue; }
      const key = (item.propertyName ?? item.name).text;
      this.bind(item.name, value.kind === "object" ? value.fields[key] ?? literal(undefined) : unknown("unresolved object"), module, state);
    }
  }
  expression(node, module, state, depth) {
    if (!node) return literal(undefined);
    if (depth > this.maxDepth) { state.blockers.push("expression depth budget"); return unknown("depth"); }
    if (++this.steps > this.maxSteps) { state.blockers.push("step budget"); return unknown("budget"); }
    if (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node)) return this.expression(node.expression, module, state, depth);
    if (ts.isArrowFunction(node) || ts.isFunctionExpression(node) || ts.isFunctionDeclaration(node)) return { kind: "function", node, module, env: new Map(state.env) };
    if (ts.isIdentifier(node)) {
      const symbol = ts.isExportSpecifier(node.parent) ? module.checker.getExportSpecifierLocalTargetSymbol(node.parent) : module.checker.getSymbolAtLocation(node);
      if (!symbol && node.text === "undefined") return literal(undefined);
      if (state.env.has(symbol)) return state.env.get(symbol);
      if (module.imports.has(symbol)) {
        const value = this.imported(module, module.imports.get(symbol), state, depth);
        return value.kind === "native" && value.name === "DEV" ? literal(true) : value;
      }
      const init = module.globals.get(symbol);
      if (init && (ts.isFunctionDeclaration(init) || ts.isArrowFunction(init) || ts.isFunctionExpression(init) || ts.isObjectLiteralExpression(init) || ts.isLiteralExpression(init)))
        return this.expression(init, module, state, depth + 1);
      return unknown("unresolved/global initializer");
    }
    if (node.kind === ts.SyntaxKind.TrueKeyword || node.kind === ts.SyntaxKind.FalseKeyword) return literal(node.kind === ts.SyntaxKind.TrueKeyword);
    if (node.kind === ts.SyntaxKind.NullKeyword) return literal(null);
    if (ts.isStringLiteral(node) || ts.isNumericLiteral(node)) return literal(ts.isNumericLiteral(node) ? Number(node.text) : node.text);
    if (ts.isArrayLiteralExpression(node)) {
      const items = [];
      for (const element of node.elements) {
        const value = this.expression(ts.isSpreadElement(element) ? element.expression : element, module, state, depth);
        if (!ts.isSpreadElement(element)) items.push(value);
        else if (value.kind === "array") items.push(...value.items);
        else { state.blockers.push("unmodeled array spread positions"); return unknown("array spread"); }
      }
      return { kind: "array", items };
    }
    if (ts.isObjectLiteralExpression(node)) {
      const fields = {}; let opaque = false;
      for (const item of node.properties) if (ts.isPropertyAssignment(item) && !ts.isComputedPropertyName(item.name)) fields[item.name.text] = this.expression(item.initializer, module, state, depth);
      else if (ts.isShorthandPropertyAssignment(item)) fields[item.name.text] = this.expression(item.name, module, state, depth);
      else {
        state.unknowns.push("unmodeled object field"); opaque = true;
        if (ts.isGetAccessorDeclaration(item) || ts.isSetAccessorDeclaration(item) || item.name && ts.isComputedPropertyName(item.name))
          state.blockers.push("unmodeled object accessor/computed field");
      }
      return opaque ? unknown("opaque object fields/spread") : objectValue(fields);
    }
    if (ts.isPropertyAccessExpression(node) || ts.isElementAccessExpression(node)) {
      const value = this.expression(node.expression, module, state, depth), key = ts.isPropertyAccessExpression(node) ? node.name.text : this.expression(node.argumentExpression, module, state, depth).value;
      if (value.kind === "namespace" && ts.isPropertyAccessExpression(node)) return this.imported(value.module, { specifier: value.specifier, name: key }, state, depth);
      if (value.kind === "array") return key === "slice" ? { kind: "array-slice", items: value.items } : value.items[key] ?? unknown("array member");
      if (value.kind === "object") return value.fields[key] ?? literal(undefined);
      return unknown("unresolved member");
    }
    if (ts.isPrefixUnaryExpression(node)) {
      if ([ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(node.operator)) state.blockers.push("unmodeled increment/decrement");
      const value = this.expression(node.operand, module, state, depth);
      return node.operator === ts.SyntaxKind.ExclamationToken && truth(value) !== null ? literal(!truth(value)) : unknown("unary");
    }
    if (ts.isPostfixUnaryExpression(node)) { state.blockers.push("unmodeled increment/decrement"); return unknown("mutation"); }
    if (ts.isTypeOfExpression(node)) {
      const values = choices(this.expression(node.expression, module, state, depth));
      const types = values.map(value => value.kind === "literal" ? typeof value.value :
        ["function", "native", "callable", "accessor", "setter"].includes(value.kind) ? "function" :
        ["object", "array"].includes(value.kind) ? "object" : null);
      return types.every(type => type !== null && type === types[0]) ? literal(types[0]) : unknown("unresolved typeof");
    }
    if (ts.isVoidExpression(node)) { this.expression(node.expression, module, state, depth); return literal(undefined); }
    if (ts.isConditionalExpression(node)) {
      const test = truth(this.expression(node.condition, module, state, depth));
      if (test !== null) return this.expression(test ? node.whenTrue : node.whenFalse, module, state, depth);
      const branches = [fork(state), fork(state)], values = [node.whenTrue, node.whenFalse].map((item, i) => this.expression(item, module, branches[i], depth));
      this.merge(branches, state); return union(values);
    }
    if (ts.isBinaryExpression(node)) {
      const left = this.expression(node.left, module, state, depth), op = node.operatorToken.kind;
      if (op === ts.SyntaxKind.EqualsToken && ts.isIdentifier(node.left)) {
        const right = this.expression(node.right, module, state, depth); this.bind(node.left, right, module, state); return right;
      }
      if (op >= ts.SyntaxKind.FirstAssignment && op <= ts.SyntaxKind.LastAssignment) state.blockers.push("unmodeled member/compound assignment");
      if ([ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(op)) {
        const test = op === ts.SyntaxKind.QuestionQuestionToken ? left.kind === "literal" ? left.value != null : null : truth(left);
        const takeRight = op === ts.SyntaxKind.AmpersandAmpersandToken ? test : test === null ? null : !test;
        if (takeRight === false) return left;
        if (takeRight === true) return this.expression(node.right, module, state, depth);
        const rightState = fork(state), right = this.expression(node.right, module, rightState, depth); this.merge([fork(state), rightState], state); return union([left, right]);
      }
      const right = this.expression(node.right, module, state, depth);
      if ([ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken].includes(op) && left.kind === "literal" && right.kind === "literal")
        return literal(op === ts.SyntaxKind.EqualsEqualsEqualsToken ? left.value === right.value : left.value !== right.value);
      return op === ts.SyntaxKind.CommaToken ? right : unknown("unmodeled binary operation");
    }
    if (ts.isCallExpression(node)) {
      const target = this.expression(node.expression, module, state, depth), args = node.arguments.map(item => this.expression(item, module, state, depth));
      state.callNode = node; state.callModule = module;
      if (node.questionDotToken || node.arguments.some(ts.isSpreadElement)) { state.unknowns.push("optional/spread call"); return unknown("optional/spread"); }
      return this.invoke(target, args, state, depth + 1);
    }
    state.unknowns.push(`unmodeled expression ${ts.SyntaxKind[node.kind]}`); return unknown(ts.SyntaxKind[node.kind]);
  }
  statements(statements, module, states, depth) {
    for (const node of statements) {
      const next = [];
      for (const state of states) {
        if (state.returned) { next.push(state); continue; }
        if (ts.isReturnStatement(node)) { state.returnValue = this.expression(node.expression, module, state, depth); state.returned = true; }
        else if (ts.isVariableStatement(node)) for (const item of node.declarationList.declarations) this.bind(item.name, this.expression(item.initializer, module, state, depth), module, state);
        else if (ts.isFunctionDeclaration(node) && node.name) this.bind(node.name, this.expression(node, module, state, depth), module, state);
        else if (ts.isExpressionStatement(node)) this.expression(node.expression, module, state, depth);
        else if (ts.isBlock(node)) { next.push(...this.statements(node.statements, module, [state], depth)); continue; }
        else if (ts.isIfStatement(node)) {
          const test = truth(this.expression(node.expression, module, state, depth));
          const selected = test === null ? [node.thenStatement, node.elseStatement] : [test ? node.thenStatement : node.elseStatement];
          for (const branch of selected) next.push(branch ? this.statements(ts.isBlock(branch) ? branch.statements : [branch], module, [fork(state)], depth).flat() : fork(state));
          continue;
        } else if (!ts.isEmptyStatement(node)) state.blockers.push(`unmodeled control ${ts.SyntaxKind[node.kind]}`);
        next.push(state);
      }
      states = next.flat();
      if (states.length > this.maxPaths) return [{ ...states[0], blockers: [...states[0].blockers, "path budget"], returned: false }];
    }
    return states;
  }
  extract(path, name, args = null) {
    this.steps = 0;
    const module = this.module(path), state = { env: new Map(), owned: false, operations: [], unknowns: [], blockers: [] };
    const target = this.exportValue(module, name, state, 0);
    if (target.kind !== "function") return { name, behavior: {}, evidence: [], gaps: ["export is not a resolved function"], steps: this.steps };
    const value = this.invoke(target, args ?? target.node.parameters.map((_, index) => unknown(`parameter ${index}`)), state, 0);
    const returns = choices(value), behavior = {}, evidence = [...state.operations];
    if (!state.blockers.length) {
      if (returns.every(item => item.kind === "accessor")) { behavior.returns = "accessor"; evidence.push(...returns.map(item => item.evidence)); }
      else if (returns.every(item => item.kind === "array")) {
        const length = Math.min(...returns.map(item => item.items.length));
        const members = Array.from({ length }, (_, index) => index).filter(index =>
          returns.every(item => choices(item.items[index]).every(value => value.kind === "accessor")));
        if (members.length) {
          behavior.returns = members.length === 1 && members[0] === 0 ? "tuple0" : "tuple";
          if (behavior.returns === "tuple") behavior.accessorMembers = members;
          for (const index of members) evidence.push(...returns.flatMap(item => choices(item.items[index]).map(value => value.evidence)));
        }
      }
      if (state.operations.some(item => item.kind === "effect")) behavior.owner = "effect";
      else if (state.operations.some(item => item.kind === "cleanup")) behavior.owner = "cleanup";
    }
    return { name, behavior, evidence: [...new Map(evidence.map(item => [JSON.stringify(item), item])).values()],
      gaps: [...new Set([...state.blockers, ...state.unknowns])], returnKinds: returns.map(item => item.kind), steps: this.steps };
  }
}
