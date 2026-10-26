#!/usr/bin/env node
// Groups a sweep's uncertifiable results by cause, and maps each cause to the
// reproduction that stands for it (fixtures/uncertifiable-repro, ADR 0404).
//
//   node scripts/uncertifiable-causes.mjs --report <sweep.json>
//     [--closed-report <sweep.json>] [--cases fixtures/uncertifiable-repro/cases.json]
//     [--json <classified.json>] [--samples <n>] [--cause <id>]
//
// The checker's JSON does not name why a result is uncertifiable (an SC2001
// write's evidence always reads "no compiler or semantic fact places where
// this write runs"), so the cause is read from the code around the site with
// TypeScript's own symbol resolution: the project's premises (an unexcluded
// synchronous dispatch), the function that holds the site, how its value is
// used (an object member, a component prop, a package callback, a write after
// an await), the census of the component holding a read. `--closed-report` is
// a second sweep of the same corpus with `--program-boundary closed`
// (scripts/corpus-sweep.mjs): a result it no longer holds is attributed to
// the open program boundary (ADR 0402), whatever else is true of it.
//
// A cause is a heuristic label for grouping and for picking what to
// reproduce, never evidence about the code: the checker's result is what a
// reproduction must match (rule, analysis context, message family), and the
// classifier is run over the reproduction set too, so a case and its corpus
// source can be compared by the same reading.
//
// Output: one row per cause (count, apps, the case that reproduces it, sample
// sites), and with `--cases` the share of the corpus's results that some case
// reproduces. `--json` writes every classified result.
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ts = createRequire(join(repo, "packages/cli/package.json"))("typescript");

/** The message family: the wording with names, quotes and numbers taken out. */
export function messageFamily(message) {
  const rules = [
    [/^setter .* is called in .*, but no compiler or semantic fact places where this code runs/, "unplaced-write"],
    [/is read (through \S+ )?in a callback written in .* and passed to a consumer that is not proven to invoke it/, "callback-to-unproven-consumer"],
    [/is read (directly|through \S+) in .*which does not track.*call sites cannot be enumerated/s, "direct-read-callers-unenumerable"],
    [/is read (directly|through \S+) in .*which does not track/s, "direct-read"],
    [/is read (through \S+ )?in .*, but no compiler or semantic fact places where this code runs/, "unplaced-read"],
    [/receives a type-correct callback whose exact synchronous body cannot be resolved/, "leaf-callback-body"],
    [/invokes \.\S+ on a caller-supplied value/, "parameter-member-call"],
    [/^the runtime target of .* cannot be selected exactly/, "runtime-target-unselected"],
    [/hands .* to an element as an event handler, and the function it holds is not resolved exactly/s, "handler-unresolved"],
    [/this function is exported, so solid-checker cannot prove its callers provide/, "exported-function-callers"],
    [/returns a cleanup function in a scope with no owner/, "settled-cleanup-unowned"],
    [/reads .* once during DOM setup/, "handler-frozen"],
    [/is (created|called) without a reactive owner/, "unowned-primitive"],
    [/may run a getter that a package analyzed as source defines/, "dependency-source-getter"],
    [/return value depends on a reactive condition, but a component body runs once/, "conditional-return"],
    [/runs where a reactive read would not be tracked, but its only evidence/, "type-only-source"],
    [/is never traced to a render or hydrate root/, "unrooted-read"],
    [/is called without an effect function/, "missing-effect-function"],
    [/client call to a server function has an unresolved rich-argument/, "server-function-argument"],
    [/^\S+\(\) is called in .*, but no compiler or semantic fact places where this code runs/, "unplaced-call"],
    [/^action .* is called in .*, but no compiler or semantic fact places where this code runs/, "unplaced-action"],
    [/destructuring props unwraps each property once/, "destructured-props"],
    [/is passed to .*, whose reactive behaviour is not described/, "package-callee"],
    [/is used as a value here, so the calls that reach it are not followed/, "primitive-value"],
    [/is called on the result of \S+, so the value it returns is not/, "primitive-result-method"],
    [/is a reactive props object and is written through directly/, "props-write"],
    [/the Solid compiler refused/, "compiler-refused"],
    [/may be read in .*; whether apply executes is unresolved/, "apply-unresolved"],
    [/is not analyzed from source here/, "package-source-unavailable"],
  ];
  for (const [pattern, family] of rules) if (pattern.test(message)) return family;
  return message
    .replace(/"[^"]*"/g, "_")
    .replace(/`[^`]*`/g, "_")
    .replace(/\b\d+\b/g, "N")
    .split(/[;,]/)[0]
    .slice(0, 80);
}

/**
 * The analysis context when it names a kind of gap (`handler-spread-unresolved`,
 * `parameter-use-unmodeled`); one that names the enclosing function or
 * component (`TeamSettingsRoute`, `owned scope`) is a place, and drops out.
 */
export const contextKind = (context) => {
  if (context == null) return "";
  if (/while evaluating a condition/.test(context)) return "condition";
  const head = context.split(":")[0];
  return /^[a-z]+(-[a-z]+)+$/.test(head) ? head : "";
};

// ---------------------------------------------------------------------------
// TypeScript reading of one project.

const isFunction = (node) =>
  ts.isArrowFunction(node) ||
  ts.isFunctionExpression(node) ||
  ts.isFunctionDeclaration(node) ||
  ts.isMethodDeclaration(node) ||
  ts.isGetAccessorDeclaration(node) ||
  ts.isSetAccessorDeclaration(node) ||
  ts.isConstructorDeclaration(node);

const hasModifier = (node, kind) => (ts.canHaveModifiers(node) ? ts.getModifiers(node) ?? [] : []).some((m) => m.kind === kind);

const peel = (node) => {
  let parent = node.parent;
  while (
    parent &&
    (ts.isParenthesizedExpression(parent) ||
      ts.isAsExpression(parent) ||
      ts.isNonNullExpression(parent) ||
      ts.isSatisfiesExpression(parent) ||
      ts.isTypeAssertionExpression(parent))
  ) {
    node = parent;
    parent = parent.parent;
  }
  return { node, parent };
};

const enclosingFunction = (node) => {
  let parent = node.parent;
  while (parent && !isFunction(parent)) parent = parent.parent;
  return parent;
};

function functionName(fn) {
  if (fn.name && ts.isIdentifier(fn.name)) return fn.name.text;
  const { parent } = peel(fn);
  if (parent && ts.isVariableDeclaration(parent) && ts.isIdentifier(parent.name)) return parent.name.text;
  if (parent && (ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent)) && parent.name) return parent.name.getText();
  return null;
}

function containsJsx(node) {
  let found = false;
  const visit = (child) => {
    if (found) return;
    if (ts.isJsxElement(child) || ts.isJsxSelfClosingElement(child) || ts.isJsxFragment(child)) {
      found = true;
      return;
    }
    if (isFunction(child)) return;
    ts.forEachChild(child, visit);
  };
  if (fn_body(node)) visit(fn_body(node));
  return found;
}
const fn_body = (fn) => fn.body;

const isComponentLike = (fn) => /^[A-Z]/.test(functionName(fn) ?? "") && containsJsx(fn);

/** Where a declaration lives: the project, a package, or a TypeScript lib file. */
function origin(declaration) {
  const file = declaration.getSourceFile().fileName;
  const lib = file.match(/\/typescript\/lib\/(lib\.[\w.]+)\.d\.ts$/);
  if (lib) return lib[1].startsWith("lib.dom") ? "lib-dom" : "lib-es";
  const packages = file.match(/node_modules\/((?:@[^/]+\/)?[^/]+)/g);
  if (packages) return `package:${packages[packages.length - 1].replace("node_modules/", "")}`;
  return "project";
}

class Project {
  constructor(root, tsconfig) {
    const path = join(root, tsconfig);
    const config = ts.readConfigFile(path, ts.sys.readFile);
    const parsed = ts.parseJsonConfigFileContent(config.config ?? {}, ts.sys, dirname(path));
    this.options = { ...parsed.options, noEmit: true, skipLibCheck: true };
    this.roots = parsed.fileNames;
    this.build(this.roots);
    this.references = new Map();
  }
  build(roots) {
    this.program = ts.createProgram({ rootNames: roots, options: this.options, oldProgram: this.program });
    this.checker = this.program.getTypeChecker();
    this.sources = this.program.getSourceFiles().filter((file) => !file.isDeclarationFile && !file.fileName.includes("/node_modules/"));
  }
  file(path) {
    let file = this.program.getSourceFile(path);
    if (!file && existsSync(path)) {
      this.roots = [...this.roots, path];
      this.build(this.roots);
      this.references.clear();
      file = this.program.getSourceFile(path);
    }
    return file;
  }
  symbol(node) {
    let symbol = this.checker.getSymbolAtLocation(node);
    if (symbol && symbol.flags & ts.SymbolFlags.Alias) {
      try {
        symbol = this.checker.getAliasedSymbol(symbol);
      } catch {
        /* an unresolved alias stays the alias */
      }
    }
    return symbol;
  }
  /**
   * Every identifier in project files that resolves to `symbol`'s declaration,
   * declarations excluded; a default import names it under any local name.
   */
  referencesOf(symbol) {
    if (this.references.has(symbol)) return this.references.get(symbol);
    const declaration = symbol.declarations?.[0];
    const found = [];
    for (const file of this.sources) {
      const names = new Set([symbol.name]);
      for (const statement of file.statements) {
        if (ts.isImportDeclaration(statement) && statement.importClause?.name) names.add(statement.importClause.name.text);
      }
      if (![...names].some((name) => file.text.includes(name))) continue;
      const visit = (node) => {
        if (ts.isIdentifier(node) && names.has(node.text)) {
          const parent = node.parent;
          const declares =
            (ts.isVariableDeclaration(parent) || ts.isFunctionDeclaration(parent) || ts.isBindingElement(parent) || ts.isParameter(parent) || ts.isImportClause(parent)) &&
            parent.name === node;
          const resolved = ts.isShorthandPropertyAssignment(parent) && parent.name === node
            ? this.checker.getShorthandAssignmentValueSymbol(parent)
            : this.symbol(node);
          if (!declares && resolved && (resolved === symbol || resolved.declarations?.[0] === declaration)) found.push(node);
        }
        ts.forEachChild(node, visit);
      };
      visit(file);
    }
    this.references.set(symbol, found);
    return found;
  }
  callee(call) {
    const expression = call.expression;
    const target = ts.isPropertyAccessExpression(expression) ? expression.name : expression;
    return {
      name: ts.isPropertyAccessExpression(expression) ? expression.name.text : expression.getText().slice(0, 40),
      origin: this.originOf(target),
    };
  }
  /** Where the value an identifier, or a member name, is declared. */
  originOf(node) {
    const declaration = this.symbol(node)?.declarations?.[0];
    return declaration ? origin(declaration) : "unresolved";
  }
}

/** How the value at `node` (a function literal or a reference to one) is used. */
function useOf(project, node, depth = 0) {
  const { node: self, parent } = peel(node);
  if (!parent) return { kind: "none" };
  if (ts.isCallExpression(parent) && parent.expression === self) return { kind: "called", call: parent };
  if (ts.isCallExpression(parent)) return { kind: "argument", ...project.callee(parent), call: parent };
  if (ts.isNewExpression(parent)) return { kind: "new-argument" };
  if (ts.isJsxExpression(parent) && parent.parent && ts.isJsxAttribute(parent.parent)) {
    const attribute = parent.parent;
    const tag = attribute.parent.parent.tagName;
    const intrinsic = ts.isIdentifier(tag) && /^[a-z]/.test(tag.text);
    const name = attribute.name.getText();
    if (intrinsic) return { kind: /^on/.test(name) ? "intrinsic-handler" : "intrinsic-attribute", name };
    const declaration = project.symbol(ts.isPropertyAccessExpression(tag) ? tag.name : tag)?.declarations?.[0];
    return { kind: "component-prop", name, declaration, origin: project.originOf(ts.isPropertyAccessExpression(tag) ? tag.name : tag) };
  }
  if (ts.isJsxExpression(parent)) return { kind: "jsx-child" };
  if (ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent)) return { kind: "object-member", object: parent.parent };
  if (ts.isMethodDeclaration(self) && ts.isObjectLiteralExpression(parent)) return { kind: "object-member", object: parent };
  if (ts.isVariableDeclaration(parent)) return ts.isIdentifier(parent.name) ? { kind: "bound", declaration: parent } : { kind: "destructured" };
  if (ts.isReturnStatement(parent) || (ts.isArrowFunction(parent) && parent.body === self)) return { kind: "returned" };
  if (ts.isBinaryExpression(parent) && parent.operatorToken.kind === ts.SyntaxKind.EqualsToken) return { kind: "assigned" };
  if (ts.isArrayLiteralExpression(parent)) return { kind: "array-element" };
  if (ts.isExportAssignment(parent) || ts.isExportSpecifier(parent)) return { kind: "exported" };
  if ((ts.isConditionalExpression(parent) || ts.isBinaryExpression(parent)) && depth < 4) return useOf(project, parent, depth + 1);
  if (ts.isSpreadAssignment(parent) || ts.isSpreadElement(parent)) return { kind: "spread" };
  return { kind: ts.SyntaxKind[parent.kind] };
}

/** The uses of a function's value: each reference of its binding, or its own position. */
function usesOf(project, fn) {
  let binding = null;
  if (ts.isFunctionDeclaration(fn) && fn.name) binding = fn.name;
  const own = useOf(project, fn);
  if (own.kind === "bound" && ts.isIdentifier(own.declaration.name)) binding = own.declaration.name;
  if (ts.isMethodDeclaration(fn) && !ts.isObjectLiteralExpression(fn.parent)) return [{ kind: "class-member" }];
  if (!binding) return [own];
  const symbol = project.symbol(binding);
  if (!symbol) return [{ kind: "unresolved" }];
  const uses = project.referencesOf(symbol).map((reference) => ({ ...useOf(project, reference), node: reference }));
  const statement = own.kind === "bound" ? own.declaration.parent?.parent : fn;
  if (statement && hasModifier(statement, ts.SyntaxKind.ExportKeyword)) uses.push({ kind: "exported" });
  return uses.length ? uses : [{ kind: "unreferenced" }];
}

/** What happens to an object literal: returned from a factory, exported, bound, passed. */
function objectFate(project, object) {
  const use = useOf(project, object);
  if (use.kind === "returned") {
    // An options object a thunk returns to its consumer (`useQuery(() => ({ queryFn }))`).
    const thunk = enclosingFunction(object);
    const handed = thunk && useOf(project, thunk);
    if (handed?.kind === "argument") return `options-thunk:${handed.origin === "project" ? "project" : handed.origin.startsWith("package") ? "package" : handed.origin}`;
    return "factory-result";
  }
  if (use.kind === "bound") {
    const statement = use.declaration.parent?.parent;
    if (statement && hasModifier(statement, ts.SyntaxKind.ExportKeyword)) return "exported-object";
    const holder = enclosingFunction(object);
    const name = use.declaration.name;
    const references = project.referencesOf(project.symbol(name) ?? { name: "" });
    if (references.some((reference) => ["returned", "exported"].includes(useOf(project, reference).kind))) return "factory-result";
    if (references.some((reference) => useOf(project, reference).kind === "component-prop")) return "context-or-prop-value";
    return holder ? "local-object" : "module-object";
  }
  if (use.kind === "component-prop") return "context-or-prop-value";
  if (use.kind === "argument") return `argument-of:${use.origin === "project" ? "project" : use.origin.startsWith("package") ? "package" : use.origin}`;
  if (use.kind === "object-member") return objectFate(project, use.object);
  return use.kind;
}

/** Whether `site` runs after a possible suspension of `fn`. */
export function afterSuspension(fn, site) {
  if (!fn || !hasModifier(fn, ts.SyntaxKind.AsyncKeyword)) return false;
  let found = false;
  const visit = (node) => {
    if (found || node.pos >= site.pos) return;
    if (node !== fn && isFunction(node)) return;
    if ((ts.isAwaitExpression(node) || (ts.isForOfStatement(node) && node.awaitModifier)) && node.end <= site.pos) found = true;
    ts.forEachChild(node, visit);
  };
  visit(fn.body ?? fn);
  if (found) return true;
  // A loop that may suspend anywhere in its body has suspended before the
  // site's second pass, even when the await is written after the site (the
  // back edge). A for-initializer and a for-of/in expression run once.
  for (let child = site, node = site.parent; node && node !== fn; child = node, node = node.parent) {
    if (isFunction(node)) return false;
    if (!ts.isIterationStatement(node, false) || !suspends(node)) continue;
    const once = ts.isForStatement(node) ? node.initializer : ts.isForInStatement(node) || ts.isForOfStatement(node) ? node.expression : undefined;
    if (child !== once) return true;
  }
  return false;
}

/** Whether `node` holds an await (or is a `for await`) outside nested functions. */
function suspends(node) {
  if (ts.isForOfStatement(node) && node.awaitModifier) return true;
  let found = false;
  const visit = (child) => {
    if (found || isFunction(child)) return;
    if (ts.isAwaitExpression(child) || (ts.isForOfStatement(child) && child.awaitModifier)) found = true;
    else ts.forEachChild(child, visit);
  };
  ts.forEachChild(node, visit);
  return found;
}

/** A callback run after a promise settles: `.then`, `.catch`, `.finally`. */
const isPromiseContinuation = (use) =>
  use.kind === "argument" && ["then", "catch", "finally"].includes(use.name) && use.origin !== "project";

/** A synchronous library callback (`forEach`, `map`, ...): its caller's place is its place. */
const isSynchronousLibraryCallback = (use) =>
  use.kind === "argument" && use.origin === "lib-es" && !["then", "catch", "finally"].includes(use.name);

/** The census of the component `fn`: how every reference of its binding renders it. */
function componentCensus(project, fn) {
  const uses = usesOf(project, fn);
  const tags = [];
  const other = [];
  for (const use of uses) {
    if (use.kind === "JsxOpeningElement" || use.kind === "JsxSelfClosingElement" || use.kind === "JsxClosingElement") tags.push(use);
    else if (!["ImportSpecifier", "ImportClause", "exported", "ExportSpecifier", "TypeQuery"].includes(use.kind)) other.push(use);
  }
  return { tags, other, exported: uses.some((use) => use.kind === "exported") };
}

function jsxTagSpreads(use) {
  const element = use.node?.parent;
  const attributes = element?.attributes?.properties ?? [];
  return attributes.some((attribute) => ts.isJsxSpreadAttribute(attribute));
}

/**
 * The placement cause of a site held by function `fn` (writes and reads
 * alike). A helper that is only called takes the cause of its callers'
 * positions (`via-callers:<cause>`), followed `depth` calls up.
 */
function placementCause(project, fn, site, depth = 0, seen = new Set()) {
  for (let level = 0, current = fn; level < 6; level++) {
    if (!current) return "module-level";
    const uses = usesOf(project, current);
    if (level === 0 && afterSuspension(current, site)) return "after-await";
    if (uses.some(isPromiseContinuation)) return "after-await";
    if (uses.length === 1 && isSynchronousLibraryCallback(uses[0])) {
      current = enclosingFunction(current);
      continue;
    }
    if (isComponentLike(current) && level === 0) return "component-body";
    const kinds = new Set(uses.map((use) => use.kind));
    const object = uses.find((use) => use.kind === "object-member");
    if (object) return `object-member:${objectFate(project, object.object)}`;
    if (kinds.has("component-prop")) {
      const prop = uses.find((use) => use.kind === "component-prop");
      if (prop.origin !== "project") return "package-component-prop";
      return `component-prop>${childInvocationCause(project, prop.declaration, prop.name, depth, seen)}`;
    }
    if (kinds.has("intrinsic-handler")) return "intrinsic-handler";
    const argument = uses.find((use) => use.kind === "argument");
    if (argument) {
      // A callback handed to a lib.dom function (a timer, an observer) after
      // its caller may have suspended runs on that continuation's stack.
      const caller = enclosingFunction(argument.call);
      if (argument.origin === "lib-dom" && caller && afterSuspension(caller, argument.call)) return "after-await";
      if (["addEventListener", "removeEventListener"].includes(argument.name) && argument.origin.startsWith("lib")) return "listener-callback";
      if (argument.origin === "project") return "project-function-argument";
      if (argument.origin.startsWith("package")) return "package-callback";
      return `${argument.origin}-callback`;
    }
    if (kinds.has("called")) {
      if (depth >= 3 || seen.has(current)) return "helper-call-chain";
      seen.add(current);
      const causes = new Set(
        uses
          .filter((use) => use.kind === "called")
          .map((use) => placementCause(project, enclosingFunction(use.call), use.call, depth + 1, seen).replace(/^via-callers:/, "")),
      );
      if (kinds.has("exported") && !uses.some((use) => use.kind !== "called" && use.kind !== "exported")) causes.add("exported");
      const ordered = [...causes].sort();
      return `via-callers:${ordered.length === 1 ? ordered[0] : ordered.includes("after-await") ? "after-await" : "mixed"}`;
    }
    if (kinds.has("returned")) return "returned-closure";
    if (kinds.has("unreferenced") || (kinds.size === 1 && kinds.has("exported"))) return "unreferenced-function";
    return `other:${[...kinds][0]}`;
  }
  return "deep-chain";
}

const VIEW_CALLS = new Set(["omit", "merge", "splitProps", "mergeProps"]);

/**
 * Where a project component invokes the prop `name` it is handed: the child's
 * side of ADR 0400's walk through `props.<name>`. Each invocation is placed
 * like any other site (an intrinsic handler is a delivery); the prop is
 * followed through `omit`/`merge`/`splitProps` views (`view:`), a destructured
 * binding (`destructured:`), a spread or attribute into another project
 * component (`forwarded:`). `placed` means every invocation is a delivery, so
 * the gap is the path, not the place.
 */
function childInvocationCause(project, declaration, name, depth, seen, marks = new Set()) {
  let fn = declaration;
  if (fn && ts.isVariableDeclaration(fn)) fn = fn.initializer && isFunction(fn.initializer) ? fn.initializer : null;
  if (!fn || !isFunction(fn)) return "component-not-a-function";
  if (depth >= 3 || seen.has(fn)) return "deep-chain";
  seen.add(fn);
  const parameter = fn.parameters[0];
  if (!parameter) return "no-props-parameter";
  const views = new Set();
  const locals = new Set();
  if (ts.isIdentifier(parameter.name)) views.add(parameter.name.text);
  else if (ts.isObjectBindingPattern(parameter.name)) {
    for (const element of parameter.name.elements) {
      const key = element.propertyName?.getText() ?? element.name.getText();
      if (key === name && ts.isIdentifier(element.name)) locals.add(element.name.text);
    }
    marks.add("destructured");
  }
  const causes = new Set();
  const body = fn.body ?? fn;
  // Views of props: `const v = merge(defaults, props)`, `const [a, b] = splitProps(props, ...)`.
  const collect = (node) => {
    if (ts.isVariableDeclaration(node) && node.initializer && ts.isCallExpression(node.initializer)) {
      const call = node.initializer;
      if (VIEW_CALLS.has(project.callee(call).name) && call.arguments.some((argument) => ts.isIdentifier(argument) && views.has(argument.text))) {
        if (ts.isIdentifier(node.name)) views.add(node.name.text);
        else for (const element of node.name.elements ?? []) if (element.name && ts.isIdentifier(element.name)) views.add(element.name.text);
        marks.add("view");
      }
    }
    if (ts.isVariableDeclaration(node) && ts.isObjectBindingPattern(node.name) && node.initializer && ts.isIdentifier(node.initializer) && views.has(node.initializer.text)) {
      for (const element of node.name.elements) {
        const key = element.propertyName?.getText() ?? element.name.getText();
        if (key === name && ts.isIdentifier(element.name)) {
          locals.add(element.name.text);
          marks.add("destructured");
        }
      }
    }
    ts.forEachChild(node, collect);
  };
  collect(body);
  const invocation = (node) => {
    const { node: self, parent } = peel(node);
    if (ts.isCallExpression(parent) && parent.expression === self) {
      return placementCause(project, enclosingFunction(parent), parent, depth + 1, seen).replace(/^via-callers:/, "");
    }
    const use = useOf(project, node);
    if (use.kind === "intrinsic-handler") return "intrinsic-handler";
    if (use.kind === "component-prop" && use.origin === "project") {
      marks.add("forwarded");
      return childInvocationCause(project, use.declaration, use.name, depth + 1, seen, marks);
    }
    return use.kind === "argument" ? `passed-to-${use.origin === "project" ? "project" : "package"}` : use.kind;
  };
  const visit = (node) => {
    if (ts.isPropertyAccessExpression(node) && ts.isIdentifier(node.expression) && views.has(node.expression.text) && node.name.text === name) {
      causes.add(invocation(node));
    } else if (ts.isIdentifier(node) && locals.has(node.text) && !(ts.isBindingElement(node.parent) && node.parent.name === node)) {
      causes.add(invocation(node));
    } else if (ts.isJsxSpreadAttribute(node) && ts.isIdentifier(node.expression) && views.has(node.expression.text)) {
      const tag = node.parent.parent.tagName;
      if (!(ts.isIdentifier(tag) && /^[a-z]/.test(tag.text))) {
        const target = project.symbol(ts.isPropertyAccessExpression(tag) ? tag.name : tag)?.declarations?.[0];
        marks.add("forwarded");
        if (target && project.originOf(ts.isPropertyAccessExpression(tag) ? tag.name : tag) === "project") {
          causes.add(childInvocationCause(project, target, name, depth + 1, seen, marks));
        } else causes.add("spread-into-package-component");
      } else causes.add("intrinsic-handler");
    }
    ts.forEachChild(node, visit);
  };
  visit(body);
  const open = [...causes].filter((cause) => cause !== "intrinsic-handler" && cause !== "placed" && !cause.startsWith("view:") && !cause.startsWith("forwarded:") && !cause.startsWith("destructured:"));
  const nested = [...causes].find((cause) => /^(view|forwarded|destructured):/.test(cause));
  const tail = open.length ? open.sort()[0] : nested ? nested.replace(/^(view|forwarded|destructured):/, "") : causes.size ? "placed" : "prop-unused";
  const prefix = ["view", "forwarded", "destructured"].filter((mark) => marks.has(mark)).join("+");
  return prefix ? `${prefix}:${tail}` : tail;
}

/**
 * How the census of the component enclosing `fn` escapes (ADR 0331, 0403):
 * a reference that is not a tag, an import or an export (`component: () =>
 * Page()`, `{ component: Page }`), or a tag with a spread. Null when every
 * reference is a tag without a spread, or `fn` is in no component.
 */
function componentEscape(project, fn) {
  let component = fn;
  while (component && !isComponentLike(component)) component = enclosingFunction(component);
  if (!component) return null;
  const census = componentCensus(project, component);
  if (census.tags.some(jsxTagSpreads)) return "spread-caller";
  if (census.other.length) {
    const kinds = new Set(census.other.map((use) => use.kind));
    for (const kind of ["called", "object-member", "argument", "component-prop", "bound", "returned"]) if (kinds.has(kind)) return kind === "called" ? "called-plainly" : `${kind}-reference`;
    return `${census.other[0].kind}-reference`;
  }
  return null;
}

function propsEscapeCause(project, fn) {
  let component = fn;
  while (component && !isComponentLike(component)) component = enclosingFunction(component);
  if (!component) return "not-in-component";
  const census = componentCensus(project, component);
  if (census.tags.some(jsxTagSpreads)) return "spread-caller";
  if (census.other.length) return `non-jsx-reference:${census.other[0].kind}`;
  if (!census.tags.length) return census.exported ? "exported-unrendered" : "unrendered";
  return census.exported ? "exported-with-tag-callers" : "other";
}

/** The call at `site`: the site itself, the call inside it naming `name`, or the nearest enclosing call. */
function callAt(site, name) {
  if (ts.isCallExpression(site)) return site;
  let inner = null;
  const visit = (node) => {
    if (inner) return;
    if (ts.isCallExpression(node) && (!name || calleeName(node) === name)) {
      inner = node;
      return;
    }
    ts.forEachChild(node, visit);
  };
  ts.forEachChild(site, visit);
  if (inner) return inner;
  let call = site;
  while (call && !ts.isCallExpression(call)) call = call.parent;
  return call;
}
const calleeName = (call) =>
  ts.isPropertyAccessExpression(call.expression) ? call.expression.name.text : ts.isIdentifier(call.expression) ? call.expression.text : null;

function leafCalleeCause(project, site) {
  const call = callAt(site, null);
  if (!call) return "no-call";
  const expression = ts.isParenthesizedExpression(call.expression) ? call.expression.expression : call.expression;
  if (isFunction(expression)) return "immediately-invoked";
  const member = ts.isPropertyAccessExpression(expression);
  const symbol = project.symbol(member ? expression.name : expression);
  const declaration = symbol?.declarations?.[0];
  if (!declaration) return member ? "member-unresolved" : "unresolved";
  const where = origin(declaration);
  if (where === "project") {
    if (!member) return `project-${bindingKind(declaration)}`;
    const receiver = project.symbol(expression.expression)?.declarations?.[0];
    if (receiver && (ts.isParameter(receiver) || ts.isBindingElement(receiver))) return "project-member-of-parameter";
    return "project-member";
  }
  return `${where.startsWith("package") ? "package" : where}-${member ? "member" : "function"}`;
}

/** How a project binding that is called is declared: a function, a parameter, a destructured result. */
function bindingKind(declaration) {
  if (ts.isFunctionDeclaration(declaration)) return "function";
  if (ts.isParameter(declaration)) return "parameter";
  if (ts.isBindingElement(declaration)) {
    let pattern = declaration.parent;
    while (pattern && !ts.isParameter(pattern) && !ts.isVariableDeclaration(pattern)) pattern = pattern.parent;
    if (!pattern) return "binding";
    if (ts.isParameter(pattern)) return "parameter-member";
    const init = pattern.initializer;
    if (init && ts.isCallExpression(init)) return "destructured-call-result";
    if (init && ts.isIdentifier(init)) return "destructured-binding";
    return "destructured";
  }
  if (ts.isVariableDeclaration(declaration)) {
    const init = declaration.initializer;
    if (init && isFunction(init)) return "function";
    if (init && ts.isCallExpression(init)) return "call-result";
    return "binding";
  }
  return ts.SyntaxKind[declaration.kind];
}

function parameterMemberCause(project, site, message) {
  const member = message.match(/invokes \.(\S+) on/)?.[1];
  const call = callAt(site, message.match(/^([\w$]+) invokes/)?.[1]);
  if (!call || !member) return "unknown";
  const symbol = project.symbol(ts.isPropertyAccessExpression(call.expression) ? call.expression.name : call.expression);
  const declaration = symbol?.declarations?.find((d) => isFunction(d) || ts.isVariableDeclaration(d));
  const body = declaration && (isFunction(declaration) ? declaration : declaration.initializer);
  if (!body) return "callee-unresolved";
  let where = null;
  const visit = (node) => {
    if (where) return;
    if (ts.isPropertyAccessExpression(node) && node.name.text === member) {
      const target = project.symbol(node.name)?.declarations?.[0];
      where = target ? origin(target) : "unresolved";
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(body);
  if (!where) return "member-not-in-callee";
  return where.startsWith("package") ? "package-type" : where === "project" ? "project-type" : where;
}

function argumentKinds(project, site, message) {
  const { node, parent } = peel(site);
  if (parent && ts.isCallExpression(parent) && parent.arguments.includes(node)) return argumentKind(project, node);
  const call = callAt(site, message.match(/^the runtime target of ([\w$]+) /)?.[1]);
  if (!call) return "no-call";
  const kinds = new Set(call.arguments.map((argument) => argumentKind(project, argument)));
  for (const kind of ["object-argument", "typed-primitive-argument", "function-argument"]) if (kinds.has(kind)) return kind;
  return "literal-arguments";
}

/** An argument's kind by its declared type: the classifier's reading, not the checker's (ADR 0320). */
function argumentKind(project, argument) {
  if (ts.isLiteralExpression(argument) || argument.kind === ts.SyntaxKind.TrueKeyword || argument.kind === ts.SyntaxKind.FalseKeyword) return "literal-arguments";
  if (isFunction(argument)) return "function-argument";
  const type = project.checker.getTypeAtLocation(argument);
  const primitive = (t) =>
    t.flags & (ts.TypeFlags.StringLike | ts.TypeFlags.NumberLike | ts.TypeFlags.BooleanLike | ts.TypeFlags.Undefined | ts.TypeFlags.Null | ts.TypeFlags.EnumLike | ts.TypeFlags.BigIntLike);
  if (type.isUnion() ? type.types.every(primitive) : primitive(type)) return "typed-primitive-argument";
  return type.getCallSignatures().length ? "function-argument" : "object-argument";
}

// ---------------------------------------------------------------------------

/**
 * The kinds of the dispatch sites that withhold the project's handler proofs
 * (ADR 0356), from the report's premises, joined: `event-target` (a receiver
 * bound to `new EventTarget()`), `test-file` or `story` (a site in a test or
 * a story module), else how the function holding the site is entered. Null
 * when no site withholds.
 */
function vetoKinds(project, premises) {
  const kinds = new Set();
  for (const premise of premises) {
    const match = premise.match(/^synchronous dispatch \(ADR 0393\) at (.+):(\d+) withholds event handler/);
    if (!match) continue;
    const [, path, line] = match;
    if (/\.(test|spec)\.[cm]?[jt]sx?$/.test(path)) {
      kinds.add("test-file");
      continue;
    }
    if (/\.stories\.[cm]?[jt]sx?$/.test(path)) {
      kinds.add("story");
      continue;
    }
    const file = project.file(path);
    if (!file) {
      kinds.add("unread");
      continue;
    }
    const start = file.getPositionOfLineAndCharacter(Number(line) - 1, 0);
    let call = null;
    const visit = (node) => {
      if (call) return;
      if (ts.isCallExpression(node) && ts.isPropertyAccessExpression(node.expression) && ["dispatchEvent", "click"].includes(node.expression.name.text) && node.getStart(file) >= start) {
        call = node;
        return;
      }
      ts.forEachChild(node, visit);
    };
    visit(file);
    if (!call) {
      kinds.add("unread");
      continue;
    }
    if (constructsEventTarget(project, call.expression.expression)) {
      kinds.add("event-target");
      continue;
    }
    const fn = enclosingFunction(call);
    const uses = fn ? usesOf(project, fn) : [];
    const argument = uses.find((use) => use.kind === "argument");
    kinds.add(!fn ? "module" : argument ? `${argument.origin === "project" ? "project" : "package"}-callback` : uses.some((use) => use.kind === "called") ? "helper" : "other");
  }
  return kinds.size ? [...kinds].sort().join("+") : null;
}

/** Whether `receiver` is a binding (or a member bound by shorthand) initialized with `new EventTarget()`. */
function constructsEventTarget(project, receiver, depth = 0) {
  if (depth > 3) return false;
  const symbol = project.symbol(ts.isPropertyAccessExpression(receiver) ? receiver.name : receiver);
  for (const declaration of symbol?.declarations ?? []) {
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
      const init = declaration.initializer;
      if (ts.isNewExpression(init) && init.expression.getText() === "EventTarget") return true;
    }
    if (ts.isShorthandPropertyAssignment(declaration)) {
      const value = project.checker.getShorthandAssignmentValueSymbol(declaration);
      if (value?.declarations?.some((d) => ts.isVariableDeclaration(d) && d.initializer && ts.isNewExpression(d.initializer) && d.initializer.expression.getText() === "EventTarget")) return true;
    }
    if (ts.isPropertyAssignment(declaration) && ts.isIdentifier(declaration.initializer)) return constructsEventTarget(project, declaration.initializer, depth + 1);
  }
  return false;
}

/** The cause of one uncertifiable finding. */
export function classify(project, finding, file, { vetoed }, entry = {}) {
  const family = messageFamily(finding.message);
  const context = contextKind(finding.analysisContext);
  const start = byteToChar(file, finding.primaryLocation.startByte);
  const end = byteToChar(file, finding.primaryLocation.endByte);
  let site = file;
  const visit = (node) => {
    if (node.getStart(file) <= start && node.end >= end) {
      site = node;
      ts.forEachChild(node, visit);
    }
  };
  ts.forEachChild(file, visit);
  entry.site = site.getText(file).replace(/\s+/g, " ").slice(0, 100);
  const fn = enclosingFunction(site);
  const rule = finding.id;
  let structure;
  if (rule === "SC2001" || rule === "SC1001") {
    if (family === "direct-read-callers-unenumerable") structure = `props-escape:${propsEscapeCause(project, fn)}`;
    else structure = placementCause(project, fn, site);
    // ADR 0356: while an unexcluded dispatch site remains, no handler is
    // placed; a site the rest of the reading places as a handler is held by
    // the veto alone.
    if (vetoed && /(^|[>:])(intrinsic-handler|placed)$/.test(structure)) structure = `dispatch-veto:${vetoed}`;
  } else if (rule === "SC9012") {
    const escape = context.startsWith("parameter-") || context === "exported-parameter-member-dispatch" ? componentEscape(project, fn) : null;
    if (escape) structure = `escaped-component:${escape}`;
    else if (family === "leaf-callback-body") structure = leafCalleeCause(project, site);
    else if (context === "parameter-member-target-unresolved" || context === "exported-parameter-member-dispatch") structure = parameterMemberCause(project, site, finding.message);
    else if (context === "parameter-use-unmodeled") structure = argumentKinds(project, site, finding.message);
    else structure = "";
  } else if (rule === "SC4001") {
    if (context === "handler-spread-unresolved") {
      const name = finding.message.match(/hands (.*?) to an element/s)?.[1] ?? "";
      structure = /^props$/.test(name) ? "props" : /^omit\(|^merge\(/.test(name) ? "inline-omit" : "named-rest";
    } else if (context === "handler-member-unresolved") {
      const name = finding.message.match(/hands (.*?) to an element/s)?.[1] ?? "";
      structure = /^_?props\.[\w$]+$/.test(name) ? "props-member" : "other-member";
    } else structure = "";
  } else structure = "";
  return [rule, family, context, structure].filter((part) => part !== "").join("/");
}

const byteMaps = new WeakMap();
function byteToChar(file, byte) {
  let map = byteMaps.get(file);
  if (!map) {
    const text = file.text;
    map = new Int32Array(Buffer.byteLength(text) + 1);
    let offset = 0;
    for (let index = 0; index < text.length; index++) {
      const code = text.codePointAt(index);
      const size = code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
      for (let k = 0; k < size; k++) map[offset + k] = index;
      offset += size;
      if (code >= 0x10000) index++;
    }
    map[offset] = text.length;
    byteMaps.set(file, map);
  }
  return map[Math.min(byte, map.length - 1)];
}

// The project is part of the key: one source file can be analyzed by several
// projects of an app (a workspace's packages), each with its own answer.
const key = (row, finding) =>
  `${row.app}|${row.project}|${finding.id}|${finding.primaryLocation.path}|${finding.primaryLocation.startByte}|${finding.analysisContext}`;

/** Classifies every uncertifiable result of a sweep report. */
export function classifyReport(report, closedReport = null) {
  const closed = new Map();
  const closedApps = new Set();
  const closedPremises = new Map();
  for (const row of closedReport?.results ?? []) {
    closedApps.add(`${row.app}|${row.project}`);
    closedPremises.set(`${row.app}|${row.project}`, row.premises ?? []);
    for (const finding of row.findings) {
      if (finding.kind !== "uncertifiable") continue;
      const k = key(row, finding);
      closed.set(k, (closed.get(k) ?? 0) + 1);
    }
  }
  const classified = [];
  for (const row of report.results) {
    const findings = row.findings.filter((finding) => finding.kind === "uncertifiable");
    if (!findings.length) continue;
    const project = new Project(row.root, row.project);
    // A result the closed run keeps is held by what holds there: closing a
    // program can also exclude dispatch sites (ADR 0396's test-only helper).
    const vetoed = vetoKinds(project, closedPremises.get(`${row.app}|${row.project}`) ?? row.premises ?? []);
    for (const finding of findings) {
      const k = key(row, finding);
      const location = finding.primaryLocation;
      const entry = {
        app: row.app,
        project: row.project,
        path: relative(row.root, location.path),
        line: location.line,
        rule: finding.id,
        family: messageFamily(finding.message),
        context: contextKind(finding.analysisContext),
      };
      if (closedApps.has(`${row.app}|${row.project}`) && !(closed.get(k) > 0)) {
        entry.cause = `${finding.id}/open-program-boundary`;
      } else {
        if (closed.get(k) > 0) closed.set(k, closed.get(k) - 1);
        const file = project.file(location.path);
        entry.cause = file ? classify(project, finding, file, { vetoed }, entry) : `${finding.id}/file-not-in-program`;
      }
      classified.push(entry);
    }
  }
  return classified;
}

/** The case standing for `cause`: one of its `causes` is the key or a `/`-prefix of it. */
export function caseFor(cause, cases) {
  let best = null;
  let length = -1;
  for (const entry of cases.filter((one) => one.kind === "uncertifiable")) {
    for (const prefix of entry.causes) {
      if ((cause === prefix || cause.startsWith(`${prefix}/`)) && prefix.length > length) {
        best = entry;
        length = prefix.length;
      }
    }
  }
  return best;
}

/** Every project of the reproduction set: a directory holding a tsconfig.json, outside node_modules. */
export function setProjects(directory, prefix = "") {
  const found = [];
  for (const entry of readdirSync(join(directory, prefix), { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.name === "node_modules" || entry.name.startsWith(".")) continue;
    const relativePath = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (existsSync(join(directory, relativePath, "tsconfig.json"))) found.push(relativePath);
    found.push(...setProjects(directory, relativePath));
  }
  return found.sort();
}

/** The checker arguments a project's `.solid-checker/runtime.json` declares (scripts/coverage.mjs reads the same). */
function runtimeArguments(projectDirectory) {
  const path = join(projectDirectory, ".solid-checker", "runtime.json");
  if (!existsSync(path)) return [];
  const runtime = JSON.parse(readFileSync(path, "utf8"));
  const flags = { target: "--runtime-target", build: "--runtime-build", rendering: "--rendering", programBoundary: "--program-boundary" };
  return Object.entries(flags).flatMap(([name, flag]) => (runtime[name] === undefined ? [] : [flag, runtime[name]]));
}

/**
 * Runs the checker over every project of the set, in the shape of a corpus
 * sweep report; with `boundary`, every project asserts that program boundary
 * (the counterfactual the corpus side takes with corpus-sweep.mjs).
 */
export function sweepSet(directory, checker, typefacts, boundary = null) {
  const results = [];
  for (const project of setProjects(directory)) {
    const root = join(directory, project);
    const runtime = runtimeArguments(root).filter((argument, index, all) => argument !== "--program-boundary" && all[index - 1] !== "--program-boundary");
    const run = spawnSync(checker, ["--format", "json", "--project", join(root, "tsconfig.json"), ...(boundary ? [...runtime, "--program-boundary", boundary] : runtimeArguments(root))], {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 1 << 30,
      env: { ...process.env, SOLID_CHECKER_DAEMON: "0", SOLID_TYPEFACTS_BIN: typefacts },
    });
    if (run.status !== 0) throw new Error(`${project}: checker exited ${run.status}\n${run.stderr.slice(-2000)}`);
    const payload = JSON.parse(run.stdout);
    results.push({ app: project, root, project: "tsconfig.json", premises: payload.premises, findings: payload.findings });
  }
  return { results };
}

/**
 * The problems of each case against the set's report and its classification.
 * An uncertifiable case: every expected result is produced in the case's files
 * with its rule, message family, analysis context and cause, and every
 * expected cause is one the case stands for. A violation case: every expected
 * violation is produced there. A clean case: its files hold no finding at all.
 */
export function caseProblems(cases, classified, report) {
  const problems = [];
  const findingsIn = (entry) =>
    (report.results.find((row) => row.app === entry.project)?.findings ?? []).filter((finding) =>
      entry.files.includes(relative(join(report.results.find((row) => row.app === entry.project).root), finding.primaryLocation.path)),
    );
  for (const entry of cases) {
    if (entry.kind === "clean") {
      const found = findingsIn(entry);
      if (found.length) problems.push(`${entry.id}: a clean case has ${found.map((finding) => `${finding.kind} ${finding.id}`).join(", ")}`);
      continue;
    }
    if (entry.kind === "violation") {
      for (const expected of entry.expect) {
        const found = findingsIn(entry).some(
          (finding) =>
            finding.kind === "violation" &&
            finding.id === expected.rule &&
            messageFamily(finding.message) === expected.family &&
            contextKind(finding.analysisContext) === expected.context,
        );
        if (!found) problems.push(`${entry.id}: no ${expected.rule} ${expected.family} [${expected.context}] violation in ${entry.files.join(", ")}`);
      }
      continue;
    }
    // The first expected result is the case's own; the others are what its
    // files also produce today, pinned so that a move shows.
    const own = entry.expect[0];
    if (own && !entry.causes.some((prefix) => own.cause === prefix || own.cause.startsWith(`${prefix}/`))) {
      problems.push(`${entry.id}: its first expected cause ${own.cause} is not one the case stands for`);
    }
    for (const expected of entry.expect) {
      const found = classified.some(
        (result) =>
          result.app === entry.project &&
          entry.files.includes(result.path) &&
          result.rule === expected.rule &&
          result.family === expected.family &&
          result.context === expected.context &&
          result.cause === expected.cause,
      );
      if (!found) {
        const here = classified.filter((result) => result.app === entry.project && entry.files.includes(result.path));
        problems.push(
          `${entry.id}: no ${expected.rule} ${expected.family} [${expected.context}] ${expected.cause} in ${entry.files.join(", ")}` +
            (here.length ? `; found ${here.map((result) => `${result.rule} ${result.family} [${result.context}] ${result.cause}`).join("; ")}` : "; found nothing uncertifiable"),
        );
      }
    }
  }
  return problems;
}

/**
 * The problems of each case's corpus source: a result there has the rule,
 * message family and analysis context of the case's first expected result,
 * and a cause the case stands for (a veto case stands for a site kind the
 * source's project combines with others, `event-target+helper`).
 */
export function sourceProblems(cases, classified) {
  const problems = [];
  for (const entry of cases.filter((one) => one.kind === "uncertifiable")) {
    const expected = entry.expect[0];
    const at = classified.filter((result) => result.app === entry.source.app && result.path === entry.source.path && result.line === entry.source.line);
    const same = at.some(
      (result) =>
        result.rule === expected.rule &&
        result.family === expected.family &&
        result.context === expected.context &&
        entry.causes.some((prefix) => result.cause === prefix || result.cause.startsWith(`${prefix}/`)),
    );
    if (!same) {
      problems.push(
        `${entry.id}: corpus source ${entry.source.app}/${entry.source.path}:${entry.source.line} reads ` +
          (at.length ? at.map((result) => `${result.rule} ${result.family} [${result.context}] ${result.cause}`).join("; ") : "as nothing uncertifiable"),
      );
    }
  }
  return problems;
}

function printGroups(classified, cases, samples, only) {
  const groups = new Map();
  for (const entry of classified) {
    const group = groups.get(entry.cause) ?? { cause: entry.cause, count: 0, apps: new Map(), samples: [] };
    group.count++;
    group.apps.set(entry.app, (group.apps.get(entry.app) ?? 0) + 1);
    if (group.samples.length < samples) group.samples.push(`${entry.app}/${entry.path}:${entry.line}`);
    groups.set(entry.cause, group);
  }
  let covered = 0;
  const perCase = new Map();
  for (const group of [...groups.values()].sort((a, b) => b.count - a.count)) {
    const owner = caseFor(group.cause, cases);
    if (owner) {
      covered += group.count;
      perCase.set(owner.id, (perCase.get(owner.id) ?? 0) + group.count);
    }
    if (only && !group.cause.startsWith(only)) continue;
    const apps = [...group.apps].sort((a, b) => b[1] - a[1]).map(([app, n]) => `${app} ${n}`).join(", ");
    console.log(`${String(group.count).padStart(5)}  ${group.cause}  [${owner ? owner.id : "-"}]\n         ${apps}\n         ${group.samples.join("  ")}`);
  }
  return { covered, perCase };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = {
    report: null,
    "closed-report": null,
    repro: null,
    cases: null,
    checker: join(repo, "rust/target/release/solid-checker-rust"),
    typefacts: join(repo, "bin/solid-typefacts"),
    json: null,
    samples: "3",
    cause: null,
  };
  const argv = process.argv.slice(2);
  for (let index = 0; index < argv.length; index += 2) {
    const name = argv[index].replace(/^--/, "");
    if (!(name in options) || argv[index + 1] === undefined) throw new Error(`unknown or incomplete option ${argv[index]}`);
    options[name] = argv[index + 1];
  }
  if (!options.report && !options.repro) throw new Error("--report <sweep.json> or --repro <set directory> is required");
  const read = (path) => JSON.parse(readFileSync(resolve(path), "utf8"));
  const table = options.cases ? read(options.cases) : { cases: [] };
  const report = options.repro ? sweepSet(resolve(options.repro), resolve(options.checker), resolve(options.typefacts)) : read(options.report);
  const closed = options.repro
    ? sweepSet(resolve(options.repro), resolve(options.checker), resolve(options.typefacts), "closed")
    : options["closed-report"] ? read(options["closed-report"]) : null;
  const classified = classifyReport(report, closed);
  const { covered, perCase } = printGroups(classified, table.cases, Number(options.samples), options.cause);
  const problems = [];
  if (options.repro) problems.push(...caseProblems(table.cases, classified, report));
  else if (table.cases.length) {
    console.log(`\nreproduced: ${covered} of ${classified.length} results (${((100 * covered) / classified.length).toFixed(1)}%)`);
    for (const entry of table.cases.filter((one) => one.kind === "uncertifiable")) {
      const weight = perCase.get(entry.id) ?? 0;
      if (weight !== entry.weight) problems.push(`${entry.id}: weight ${weight} in this report, ${entry.weight} in the table`);
    }
    problems.push(...sourceProblems(table.cases, classified));
  }
  if (options.json) writeFileSync(resolve(options.json), `${JSON.stringify(classified, null, 1)}\n`);
  for (const problem of problems) console.error(problem);
  if (problems.length) process.exit(1);
}
