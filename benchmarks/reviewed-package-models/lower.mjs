// Analysis-only expansion of an explicit, source-reviewed premise into core
// primitive facts. Generated source must never be executed or certified.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { authenticateModel } from "./catalog.mjs";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const ts = createRequire(join(repo, "packages/cli/package.json"))("typescript");
const prefix = "__solidReviewed";
const aliases = { memo: `${prefix}Memo`, signal: `${prefix}Signal`, cleanup: `${prefix}Cleanup`, effect: `${prefix}Effect` };
const unalias = (checker, symbol) => symbol && (symbol.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(symbol) : symbol);
function unwrap(node) {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node)) node = node.expression;
  return node;
}
export function lower(program, source, catalog, host, authenticate = authenticateModel, specialize = null) {
  assert(["browser", "node"].includes(host), "The experiment requires an explicit host");
  assert(!source.text.includes(prefix), "Reserved experimental identifiers occur in the source");
  const checker = program.getTypeChecker(), edits = [], sites = [], unsupported = [];
  const imports = new Map(), namespaces = new Map(), admitted = new Map(), coreFactories = new Map();
  for (const statement of source.statements) {
    if (!ts.isImportDeclaration(statement) || !ts.isStringLiteral(statement.moduleSpecifier) || statement.importClause?.isTypeOnly) continue;
    if (statement.moduleSpecifier.text === "solid-js" && statement.importClause?.namedBindings && ts.isNamedImports(statement.importClause.namedBindings)) {
      for (const element of statement.importClause.namedBindings.elements) {
        const name = (element.propertyName ?? element.name).text;
        if (["createSignal", "createMemo"].includes(name)) coreFactories.set(checker.getSymbolAtLocation(element.name), name);
      }
    }
    const model = catalog.models.find(item => item.package === statement.moduleSpecifier.text);
    if (!model) continue;
    const root = authenticate(model, dirname(source.fileName));
    admitted.set(model.package, model);
    const moduleSymbol = checker.getSymbolAtLocation(statement.moduleSpecifier);
    assert(moduleSymbol, `Unresolved modeled import: ${model.package}`);
    const exports = new Map(checker.getExportsOfModule(moduleSymbol).map(symbol => [symbol.name, unalias(checker, symbol)]));
    const binding = statement.importClause?.namedBindings;
    if (binding && ts.isNamedImports(binding)) for (const element of binding.elements) {
      if (element.isTypeOnly) continue;
      const name = (element.propertyName ?? element.name).text;
      const local = checker.getSymbolAtLocation(element.name), target = unalias(checker, local);
      assert.equal(target, exports.get(name), `Export identity mismatch: ${name}`);
      assert(target?.declarations?.every(declaration => {
        const path = relative(root, declaration.getSourceFile().fileName);
        return path && !path.startsWith("..") && !path.startsWith("/");
      }), `Export declaration escapes its package: ${name}`);
      imports.set(local, { model, name });
    }
    if (binding && ts.isNamespaceImport(binding)) namespaces.set(checker.getSymbolAtLocation(binding.name), { model, exports, root });
  }
  function target(call, wrappers = new Set()) {
    const callee = unwrap(call.expression);
    if (ts.isIdentifier(callee)) {
      const symbol = checker.getSymbolAtLocation(callee), imported = imports.get(symbol);
      if (imported) return { ...imported, premiseCall: call };
      const declaration = symbol?.valueDeclaration;
      // A deliberately small local return summary. Function bodies with
      // parameters, effects, branches, async/generator execution, rebinding or
      // cross-file dispatch need richer semantic evidence and remain open.
      if (call.arguments.length || wrappers.has(symbol) || wrappers.size >= 8 || !declaration ||
        !ts.isFunctionDeclaration(declaration) || declaration.getSourceFile() !== source || declaration.parameters.length ||
        declaration.asteriskToken || declaration.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AsyncKeyword) ||
        declaration.body?.statements.length !== 1 || !ts.isReturnStatement(declaration.body.statements[0]) || !declaration.body.statements[0].expression) return null;
      let stable = true;
      const references = node => {
        if (ts.isIdentifier(node) && node !== declaration.name && checker.getSymbolAtLocation(node) === symbol &&
          !(ts.isCallExpression(node.parent) && node.parent.expression === node)) stable = false;
        ts.forEachChild(node, references);
      };
      references(source);
      if (!stable) return null;
      const returned = unwrap(declaration.body.statements[0].expression);
      if (!ts.isCallExpression(returned)) return null;
      const resolved = target(returned, new Set([...wrappers, symbol]));
      return resolved ? { ...resolved, wrapperDepth: (resolved.wrapperDepth ?? 0) + 1 } : null;
    }
    if (ts.isPropertyAccessExpression(callee) && ts.isIdentifier(callee.expression) && !callee.questionDotToken) {
      const ns = namespaces.get(checker.getSymbolAtLocation(callee.expression));
      if (ns && unalias(checker, checker.getSymbolAtLocation(callee.name)) === ns.exports.get(callee.name.text))
        return { model: ns.model, name: callee.name.text, premiseCall: call };
    }
    return null;
  }
  function addEdit(start, end, parts) { edits.push({ start, end, parts }); }
  const original = node => ({ text: node.getText(source), originalStart: node.getStart(source), copied: true });
  function stableZeroArityAccessor(arg) {
    const symbol = checker.getSymbolAtLocation(unwrap(arg));
    const declaration = symbol?.valueDeclaration;
    let value;
    if (declaration && ts.isBindingElement(declaration) && ts.isArrayBindingPattern(declaration.parent) &&
      declaration.parent.elements[0] === declaration && ts.isVariableDeclaration(declaration.parent.parent)) {
      value = unwrap(declaration.parent.parent.initializer);
      if (!ts.isCallExpression(value) || coreFactories.get(checker.getSymbolAtLocation(unwrap(value.expression))) !== "createSignal") return false;
    } else if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer) {
      value = unwrap(declaration.initializer);
      if (!ts.isCallExpression(value) || coreFactories.get(checker.getSymbolAtLocation(unwrap(value.expression))) !== "createMemo") return false;
    } else return false;
    let stable = true;
    function references(node) {
      if (ts.isIdentifier(node) && checker.getSymbolAtLocation(node) === symbol && node !== declaration.name) {
        const parent = node.parent;
        const directCall = ts.isCallExpression(parent) && parent.expression === node;
        const called = ts.isCallExpression(parent) ? target(parent) : null;
        const modeledAccess = called?.model.exports[called.name]?.[host]?.invokesArgument === 0 && parent.arguments[0] === node;
        if (!directCall && !modeledAccess) stable = false;
      }
      ts.forEachChild(node, references);
    }
    references(source);
    return stable;
  }
  function visit(node) {
    if (ts.isCallExpression(node)) {
      const resolved = target(node);
      if (resolved) {
        const { model, name } = resolved;
        const extraction = specialize?.({ model, name, node: resolved.premiseCall, source, checker, host });
        const behavior = extraction ? extraction.behavior : model.exports[name]?.[host];
        const site = { package: model.package, export: name, modelVersion: model.version, basis: model.basis ?? "reviewed-source-assumption",
          start: node.getStart(source), end: node.end, behavior, bindings: [] };
        if (extraction) site.extraction = extraction;
        if (resolved.wrapperDepth) site.wrapper = { depth: resolved.wrapperDepth, factoryStart: resolved.premiseCall.getStart(source) };
        sites.push(site);
        const refuse = reason => unsupported.push({ package: model.package, export: name, start: site.start, reason });
        if (!behavior || !Object.keys(behavior).length) { refuse("host behavior is unmodeled"); return; }
        if (resolved.wrapperDepth && (behavior.trackedCallback !== undefined || behavior.invokesArgument !== undefined)) {
          refuse("wrapper callback/invocation behavior is unsupported"); return;
        }
        if (node.questionDotToken || node.arguments.some(ts.isSpreadElement)) { refuse("optional/spread call"); return; }
        if (behavior.inert) { site.applied = true; return; }
        if (behavior.trackedCallback !== undefined && (node.arguments.length !== 1 ||
          !(ts.isArrowFunction(unwrap(node.arguments[behavior.trackedCallback])) || ts.isFunctionExpression(unwrap(node.arguments[behavior.trackedCallback]))))) {
          refuse("tracked callback currently requires one inline function argument"); return;
        }
        if (behavior.invokesArgument !== undefined) {
          const arg = node.arguments[behavior.invokesArgument];
          if (node.arguments.length !== 1 || !arg || !ts.isIdentifier(unwrap(arg))) { refuse("invocation requires one stable identifier argument"); return; }
          const type = checker.getTypeAtLocation(arg);
          if (type.isUnion() || type.flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown) || !type.getCallSignatures().length) {
            refuse("argument is not definitely callable"); return;
          }
          if (!stableZeroArityAccessor(arg)) { refuse("access requires an unescaped native zero-arity accessor; function length can change"); return; }
          addEdit(site.start, site.end, [{ text: "(", originalStart: site.start }, original(arg), { text: ")()", originalStart: site.start }]);
          site.applied = true; return;
        }
        let parent = node;
        while (parent.parent && unwrap(parent.parent) === node) parent = parent.parent;
        const declaration = ts.isVariableDeclaration(parent.parent) && parent.parent.initializer === parent ? parent.parent : null;
        const statement = declaration?.parent?.parent;
        const returned = ts.isReturnStatement(parent.parent) && ts.isBlock(parent.parent.parent) ? parent.parent : null;
        if (returned && behavior.trackedCallback !== undefined) { refuse("tracked callback wrapper returns are unsupported"); return; }
        if (returned && behavior.returns || declaration && ts.isVariableStatement(statement) && statement.declarationList.declarations.length === 1 &&
          (ts.isBlock(statement.parent) || ts.isSourceFile(statement.parent))) {
          const tuple = ["tuple0", "tuple"].includes(behavior.returns);
          const members = behavior.returns === "tuple0" ? [0] : behavior.accessorMembers;
          if (declaration && behavior.returns && !(statement.declarationList.flags & ts.NodeFlags.Const)) { refuse("mutable returned bindings are unsupported"); return; }
          if (declaration && behavior.returns === "accessor" && !ts.isIdentifier(declaration.name)) { refuse("accessor needs a direct binding"); return; }
          if (declaration && tuple && (!ts.isArrayBindingPattern(declaration.name) ||
            declaration.name.elements.some(element => !ts.isOmittedExpression(element) && (!ts.isBindingElement(element) ||
              !ts.isIdentifier(element.name) || element.initializer || element.dotDotDotToken)) ||
            !members.some(index => declaration.name.elements[index] && ts.isBindingElement(declaration.name.elements[index])))) {
            if (behavior.owner && node.arguments.length === 0) {
              // Ownership is independent of an unused/opaque tuple result.
              // Keep the real call and its published result type. No return
              // binding or reactive-read premise is supplied by this slice.
              addEdit(site.start, site.end, [{ text: `(${ownerExpression(behavior.owner)}, `, originalStart: site.start }, original(node), { text: ')', originalStart: site.start }]);
              site.applied = true; site.appliedPremises = ['owner'];
              refuse('tuple return remains opaque; zero-argument owner premise applied separately'); return;
            }
            refuse("tuple model requires a direct accessor member binding without defaults/rest"); return;
          }
          const additions = [];
          if (behavior.returns && behavior.trackedCallback === undefined) for (const arg of node.arguments)
            additions.push({ text: "void (", originalStart: site.start }, original(arg), { text: ");\n", originalStart: site.start });
          if (behavior.owner) additions.push({ text: ownerExpression(behavior.owner) + ";\n", originalStart: site.start });
          if (behavior.returns) {
            const returnType = checker.getTypeAtLocation(node);
            if (tuple && !checker.isTupleType(returnType)) { refuse("resolved return is not a tuple"); return; }
            const types = tuple ? checker.getTypeArguments(returnType) : [returnType];
            const indices = tuple ? members : [0];
            if (indices.some(index => !types[index]?.getCallSignatures().length)) { refuse("resolved return leaf is not callable"); return; }
            const flags = ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.UseFullyQualifiedType;
            const publishedType = checker.typeToString(returnType, node, flags);
            const compute = behavior.trackedCallback === undefined ? "() => undefined as never" : node.arguments[behavior.trackedCallback].getText(source);
            const memo = index => `${aliases.memo}<${checker.typeToString(checker.getReturnTypeOfSignature(types[index].getCallSignatures()[0]), node, flags)}>(${compute})`;
            const value = tuple ? `[${types.map((_, index) => indices.includes(index) ? memo(index) : "undefined as never").join(", ")}]` : memo(0);
            // Native memos carry a refresh brand that many package accessors
            // intentionally omit. Preserve the published return type, including
            // valid later rebinding, while the core fact records reactivity.
            if (returned) {
              const binding = `${prefix}Return${sites.length}`;
              addEdit(returned.getStart(source), returned.end, [...additions,
                { text: `const ${binding} = (${value} as unknown as ${publishedType});\nreturn ${binding};`, originalStart: site.start }]);
              site.bindings.push({ start: site.start, end: site.start + 1 });
            } else if (tuple) {
              const parts = [...additions];
              for (let index = 0; index < declaration.name.elements.length; index++) {
                const element = declaration.name.elements[index];
                if (!ts.isBindingElement(element)) continue;
                const leafType = checker.typeToString(types[index], node, flags);
                const modifiers = statement.modifiers?.map(modifier => modifier.getText(source)).join(" ");
                parts.push({ text: `${modifiers ? modifiers + " " : ""}const `, originalStart: site.start }, original(element.name),
                  { text: ` = (${indices.includes(index) ? memo(index) : "undefined as never"} as unknown as ${leafType});\n`, originalStart: site.start });
              }
              addEdit(statement.getStart(source), statement.end, parts);
              const bindings = indices.map(index => declaration.name.elements[index]).filter(element => element && ts.isBindingElement(element)).map(element => element.name);
              site.bindings.push(...bindings.map(binding => ({ start: binding.getStart(source), end: binding.end })));
            } else {
              addEdit(site.start, site.end, [{ text: `(${value} as unknown as ${publishedType})`, originalStart: site.start }]);
              site.bindings.push({ start: declaration.name.getStart(source), end: declaration.name.end });
            }
          }
          if (!returned && !tuple && additions.length) addEdit(statement.getStart(source), statement.getStart(source), additions);
          site.applied = true; return;
        }
        if (behavior.owner && ts.isExpressionStatement(parent.parent) && !behavior.returns) {
          const parts = [{ text: "(", originalStart: site.start }];
          for (const arg of node.arguments) parts.push({ text: "void (", originalStart: site.start }, original(arg), { text: "), ", originalStart: site.start });
          parts.push({ text: ownerExpression(behavior.owner) + ")", originalStart: site.start });
          addEdit(site.start, site.end, parts); site.applied = true; return;
        }
        // A factory used only for setup may discard a modeled return.
        if (behavior.owner && ts.isExpressionStatement(parent.parent)) {
          addEdit(site.start, site.start, [{ text: `${ownerExpression(behavior.owner)}, `, originalStart: site.start }]);
          site.applied = true; return;
        }
        refuse("unsupported factory use or expression placement"); return;
      }
      const callee = unwrap(node.expression);
      if (ts.isElementAccessExpression(callee) && ts.isIdentifier(callee.expression) && namespaces.has(checker.getSymbolAtLocation(callee.expression)))
        unsupported.push({ start: node.getStart(source), reason: "computed namespace dispatch" });
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  if (edits.length) addEdit(0, 0, [{ text: `import { createMemo as ${aliases.memo}, createSignal as ${aliases.signal}, onCleanup as ${aliases.cleanup}, createEffect as ${aliases.effect} } from "solid-js";\n`, originalStart: 0 }]);
  edits.sort((a, b) => a.start - b.start || a.end - b.end);
  let cursor = 0, text = "";
  const segments = [];
  function append(part) {
    const start = text.length; text += part.text;
    segments.push({ start, end: text.length, originalStart: part.originalStart, copied: part.copied ?? false });
  }
  for (const edit of edits) {
    assert(edit.start >= cursor, "Overlapping modeled calls are unsupported");
    append({ text: source.text.slice(cursor, edit.start), originalStart: cursor, copied: true });
    for (const part of edit.parts) append(part);
    cursor = edit.end;
  }
  append({ text: source.text.slice(cursor), originalStart: cursor, copied: true });
  return { text, segments, sites, unsupported, admitted: [...admitted.keys()] };
}
function ownerExpression(owner) {
  assert(["cleanup", "effect"].includes(owner));
  return owner === "cleanup" ? `${aliases.cleanup}(() => {})` : `${aliases.effect}(() => 0, () => {})`;
}
export function projectWarning(finding, lowered, original, modeledPath) {
  if (finding.kind !== "violation" || resolve(finding.primaryLocation.path) !== resolve(modeledPath)) return null;
  const map = location => {
    if (resolve(location.path) !== resolve(modeledPath)) return null;
    const utf16 = Buffer.from(lowered.text).subarray(0, location.startByte).toString("utf8").length;
    const segment = lowered.segments.find(item => item.start <= utf16 && utf16 < item.end);
    if (!segment) return null;
    return segment.originalStart + (segment.copied ? utf16 - segment.start : 0);
  };
  const start = map(finding.primaryLocation);
  const declarations = [...finding.relatedLocations ?? [], ...finding.evidence?.map(item => item.location) ?? []].map(map).filter(value => value !== null);
  const site = lowered.sites.find(item => item.applied && !item.behavior?.inert &&
    // An unmodeled callback in a copied argument does not inherit the premise.
    // Generated operations anchor at the call; modeled return reads carry the
    // exact binding through the checker's related semantic locations.
    (item.start === start || item.bindings.some(binding => declarations.some(position => binding.start <= position && position < binding.end))));
  if (!site) return null;
  // Surrogates expose only the premise they model. Creating a memo to expose
  // an accessor does not establish the package's own construction restrictions.
  const supported = finding.rule === "strict-read-untracked" ? site.behavior?.returns || site.behavior?.invokesArgument !== undefined :
    ["missing-owner", "leaf-owner-forbidden-call"].includes(finding.rule) ? site.behavior?.owner :
    finding.rule === "reactive-write-in-owned-scope" ? site.behavior?.trackedCallback !== undefined : false;
  if (!supported) return null;
  if (site.appliedPremises && !(['missing-owner', 'leaf-owner-forbidden-call'].includes(finding.rule) && site.appliedPremises.includes('owner'))) return null;
  const position = original.getLineAndCharacterOfPosition(start);
  const basis = site.basis ?? "reviewed-source-assumption";
  return { id: finding.id, rule: finding.rule, severity: "warning", basis, certification: false,
    package: site.package, export: site.export, modelVersion: site.modelVersion,
    message: `Under the ${basis === "source-extracted-assumption" ? "source-extracted" : "reviewed"} model for ${site.package}.${site.export}: ${finding.message}`,
    location: { path: original.fileName, startByte: Buffer.byteLength(original.text.slice(0, start)), line: position.line + 1, column: position.character + 1 },
    analyzerFindingKind: finding.kind };
}
