#!/usr/bin/env node
// Groups a sweep's uncertifiable results by cause, and maps each cause to the
// reproduction that stands for it (fixtures/uncertifiable-repro, ADR 0446).
//
//   node scripts/uncertifiable-causes.mjs --report <sweep.json>
//     [--closed-report <sweep.json>] [--noveto-report <sweep.json>] [--cases fixtures/uncertifiable-repro/cases.json]
//     [--json <classified.json>] [--samples <n>] [--cause <id>]
//     [--draw <n> --draw-out <samples.json>]
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
// the open program boundary (ADR 0444), whatever else is true of it.
// `--noveto-report` is a sweep by a scratch build whose dispatch sites
// withhold nothing (`withholds_owner_proofs` and `withholds_read_proofs` in
// indexes/event_delivery.rs answer false; never committed): in a project with
// unexcluded sites, a result it no longer holds is attributed to the dispatch
// veto (ADR 0356) and one it still holds to its other cause. Without it the
// veto is read from the site's shape, which over-attributes.
//
// A cause is a heuristic label for grouping and for picking what to
// reproduce, never evidence about the code: the checker's result is what a
// reproduction must match (rule, analysis context, message family), and the
// classifier is run over the reproduction set too, so a case and its corpus
// source can be compared by the same reading.
//
// Output: one row per cause (count, apps, the case that reproduces it, sample
// sites), and with `--cases` two shares: label coverage (results whose label
// maps to a case) and verified coverage (distinct sites under cases whose
// sampled sites a source read found to be the case's mechanism, the table's
// `verifiedWeight`), with every table weight checked against the report.
// `--draw <n> --draw-out <file>` writes each case's seeded sample of `n`
// distinct sites to read. `--json` writes every classified result.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
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
    [/^the pinned Solid \d+ compiler refuses /, "compiler-refusal"],
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

export const enclosingFunction = (node) => {
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

export const isComponentLike = (fn) => /^[A-Z]/.test(functionName(fn) ?? "") && containsJsx(fn);

/** Where a declaration lives: the project, a package, or a TypeScript lib file. */
function origin(declaration) {
  const file = declaration.getSourceFile().fileName;
  const lib = file.match(/\/typescript\/lib\/(lib\.[\w.]+)\.d\.ts$/);
  if (lib) return lib[1].startsWith("lib.dom") ? "lib-dom" : "lib-es";
  const packages = file.match(/node_modules\/((?:@[^/]+\/)?[^/]+)/g);
  if (packages) return `package:${packages[packages.length - 1].replace("node_modules/", "")}`;
  return "project";
}

export class Project {
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
      declared: this.symbol(target)?.name,
    };
  }
  /** Where the value an identifier, or a member name, is declared. */
  originOf(node) {
    const declaration = this.symbol(node)?.declarations?.[0];
    return declaration ? origin(declaration) : "unresolved";
  }
}

/** How the value at `node` (a function literal or a reference to one) is used. */
export function useOf(project, node, depth = 0) {
  const { node: self, parent } = peel(node);
  if (!parent) return { kind: "none" };
  if (ts.isCallExpression(parent) && parent.expression === self) return { kind: "called", call: parent };
  if (ts.isCallExpression(parent)) return { kind: "argument", ...project.callee(parent), call: parent };
  if (ts.isNewExpression(parent)) return { kind: "new-argument", origin: project.originOf(parent.expression) };
  if (ts.isJsxExpression(parent) && parent.parent && ts.isJsxAttribute(parent.parent)) {
    const attribute = parent.parent;
    const tag = attribute.parent.parent.tagName;
    const intrinsic = ts.isIdentifier(tag) && /^[a-z]/.test(tag.text);
    const name = attribute.name.getText();
    if (intrinsic) return { kind: /^on/.test(name) ? "intrinsic-handler" : "intrinsic-attribute", name };
    const declaration = project.symbol(ts.isPropertyAccessExpression(tag) ? tag.name : tag)?.declarations?.[0];
    return { kind: "component-prop", name, declaration, element: attribute.parent.parent, origin: project.originOf(ts.isPropertyAccessExpression(tag) ? tag.name : tag) };
  }
  if (ts.isJsxExpression(parent)) return { kind: "jsx-child" };
  if (ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent)) return { kind: "object-member", object: parent.parent, key: staticName(parent.name) };
  if (ts.isMethodDeclaration(self) && ts.isObjectLiteralExpression(parent)) return { kind: "object-member", object: parent, key: staticName(self.name) };
  if ((ts.isGetAccessorDeclaration(self) || ts.isSetAccessorDeclaration(self)) && ts.isObjectLiteralExpression(parent)) {
    return { kind: "object-accessor", object: parent, key: staticName(self.name), accessor: ts.isGetAccessorDeclaration(self) ? "get" : "set" };
  }
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
export function usesOf(project, fn) {
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

/**
 * Set while one SC1001 result is classified: a read's labels name more than a
 * write's do (what an array holding the callback is handed to, whether a
 * component prop is a context's value), and a write's labels stay as they are.
 */
let readSite = false;

/** The consumer of an array literal that holds callbacks (a read only): a prop, a return, an argument. */
function arrayFate(project, array) {
  const use = useOf(project, array);
  if (use.kind === "component-prop") return `component-prop:${use.origin === "project" ? "project" : "package"}`;
  if (use.kind === "argument") return `argument-of:${use.origin === "project" ? "project" : use.origin.startsWith("package") ? "package" : use.origin}`;
  return use.kind;
}

/** What a component prop that is handed an object is: a context's value, or a prop of a project or package component. */
function propValueFate(project, use) {
  const declaration = use.declaration;
  const init = declaration && ts.isVariableDeclaration(declaration) ? declaration.initializer : null;
  if (init && ts.isCallExpression(init) && project.callee(init).name === "createContext" && project.callee(init).origin.startsWith("package")) return "context-value";
  return `prop-value:${use.origin === "project" ? "project" : "package"}`;
}

/** The text of a property name written as an identifier or a string literal; null when computed. */
const staticName = (name) => (ts.isIdentifier(name) || ts.isStringLiteral(name) || ts.isNoSubstitutionTemplateLiteral(name) || ts.isNumericLiteral(name) ? name.text : "<computed>");

/** The rule of the finding being classified: some refinements apply to one rule's labels only, so another area's causes keep theirs. */
const scope = { rule: "", family: "" };

const originKind = (where) => (where === "project" ? "project" : where.startsWith("package") ? "package" : where);

/**
 * What the callee of `call` is declared as: a project function (with its
 * declaration), or the other thing a call can reach (a signature, a parameter,
 * a destructured binding such as a store setter, a call result).
 */
function calleeDeclaration(project, call) {
  const expression = call.expression;
  const target = ts.isPropertyAccessExpression(expression) ? expression.name : expression;
  const declaration = project.symbol(target)?.declarations?.[0];
  if (!declaration) return { kind: "unresolved" };
  if (isFunction(declaration)) return { kind: "function", fn: declaration };
  if (ts.isVariableDeclaration(declaration)) {
    let inner = declaration.initializer;
    while (inner && (ts.isParenthesizedExpression(inner) || ts.isAsExpression(inner) || ts.isSatisfiesExpression(inner))) inner = inner.expression;
    if (inner && isFunction(inner)) return { kind: "function", fn: inner };
    return { kind: inner && ts.isCallExpression(inner) ? "call-result" : "variable" };
  }
  if (ts.isBindingElement(declaration)) return { kind: "destructured-binding" };
  if (ts.isMethodSignature(declaration) || ts.isPropertySignature(declaration)) return { kind: "signature" };
  if (ts.isParameter(declaration)) return { kind: "parameter" };
  return { kind: ts.SyntaxKind[declaration.kind] };
}

/** The index of the argument of `call` that holds `node`. */
const argumentIndex = (call, node) => call.arguments.findIndex((argument) => argument.pos <= node.pos && node.end <= argument.end);

const isSolidOrgPackage = (where) => /^package:(solid-js|@solidjs\/.*)$/.test(where);

/**
 * Where a call written in `fn` (a project callee) runs, for the use of a
 * handed-in function (callee_parameter_use_role's walk of the functions
 * between the callee and the call): in the body itself (or in a callback the
 * standard library runs inline), in a callback a timer, listener or Solid
 * primitive runs (`called-in-callback`), or in a closure the callee keeps,
 * hands to another function or continues after a promise.
 */
function invocationPosition(project, callee, call) {
  for (let nested = enclosingFunction(call); nested && nested !== callee; nested = enclosingFunction(nested)) {
    if (hasModifier(nested, ts.SyntaxKind.AsyncKeyword) || nested.asteriskToken) return "called-in-async-closure";
    const use = useOf(project, nested);
    if (use.kind === "argument") {
      if (isPromiseContinuation(use)) return "called-in-promise-continuation";
      if (isSynchronousLibraryCallback(use)) continue;
      if (use.origin === "lib-dom" || isSolidOrgPackage(use.origin)) return "called-in-callback";
      return `called-in-${originKind(use.origin)}-callback`;
    }
    if (use.kind === "intrinsic-handler" || use.kind === "intrinsic-attribute") return "called-in-callback";
    if (use.kind === "bound") return "called-in-bound-helper";
    if (use.kind === "called") return "called-in-iife";
    if (use.kind === "jsx-child" || use.kind === "component-prop") return "called-in-jsx-callback";
    return `called-in-kept-closure@${keptClosureFate(project, nested)}`;
  }
  return "called-in-body";
}

/**
 * Where the closure a callee keeps goes (the `@` note after `called-in-kept-closure`,
 * kept on the label of the cases that stand for it): a member of the object the
 * callee returns, or something else (a member of an object it hands on, a binding).
 */
function keptClosureFate(project, closure) {
  const fates = new Set();
  for (const use of usesOf(project, closure)) {
    if (use.kind === "object-member") {
      const fate = objectFate(project, use.object);
      fates.add(fate === "factory-result" ? "result-member" : `member-of:${fate}`);
    } else fates.add(use.kind);
  }
  return [...fates].sort().join("+");
}

/** The declaration the identifier `node` reads as a value; the name of a shorthand property reads the binding it names, not the property. */
function valueDeclaration(project, node) {
  if (node.parent && ts.isShorthandPropertyAssignment(node.parent) && node.parent.name === node) {
    let symbol = project.checker.getShorthandAssignmentValueSymbol(node.parent);
    if (symbol && symbol.flags & ts.SymbolFlags.Alias) {
      try {
        symbol = project.checker.getAliasedSymbol(symbol);
      } catch {
        /* an unresolved alias stays the alias */
      }
    }
    return symbol?.declarations?.[0];
  }
  return project.symbol(node)?.declarations?.[0];
}

/** Where the value a binding is initialized from comes from: the origin of the function it calls, or what else it is. */
function bindingSource(project, declaration) {
  let holder = declaration;
  while (holder && ts.isBindingElement(holder)) holder = holder.parent.parent;
  if (holder && ts.isParameter(holder)) return "parameter";
  const initializer = holder && ts.isVariableDeclaration(holder) && holder.initializer ? unwrap(holder.initializer) : null;
  if (!initializer) return "none";
  if (ts.isCallExpression(initializer)) return originKind(project.originOf(ts.isPropertyAccessExpression(initializer.expression) ? initializer.expression.name : initializer.expression));
  return `other:${ts.SyntaxKind[initializer.kind]}`;
}

/**
 * The form of a value a callee calls as a member of its caller's object or
 * element, when the proof cannot show it free of `this` (the part of the
 * label after `sibling-called`): what the value is, so that a change modelling
 * one form is not credited with the others.
 */
function valueForm(project, node) {
  const value = unwrap(node);
  if (ts.isPropertyAccessExpression(value) || ts.isElementAccessExpression(value)) return "member-read";
  if (ts.isCallExpression(value)) return "call-result";
  if (isFunction(value)) return "function-expression";
  if (ts.isIdentifier(value)) {
    const declaration = valueDeclaration(project, value);
    if (!declaration) return "unresolved-identifier";
    if (ts.isFunctionDeclaration(declaration)) return "function-declaration";
    if (ts.isBindingElement(declaration)) return `${ts.isArrayBindingPattern(declaration.parent) ? "array-binding" : "object-binding"}:${bindingSource(project, declaration)}`;
    if (ts.isVariableDeclaration(declaration)) return declaration.initializer && isFunction(unwrap(declaration.initializer)) ? "function-expression" : `variable:${bindingSource(project, declaration)}`;
    if (ts.isParameter(declaration)) return "parameter";
    return `identifier:${ts.SyntaxKind[declaration.kind]}`;
  }
  return `other:${ts.SyntaxKind[value.kind]}`;
}

/** Whether `node`'s type is one a call can be made on (a function, or a function that may be absent). */
function callableValue(project, node) {
  const type = project.checker.getNonNullableType(project.checker.getTypeAtLocation(node));
  return type.getCallSignatures().length > 0 || (type.isUnion() && type.types.some((member) => member.getCallSignatures().length > 0));
}

/**
 * The forms of every member of `siblings` that a callee could call and the
 * proof cannot show free of `this` (`unsafe(name)`), except the `tracked` member
 * the walk follows, joined: a change that
 * models one form moves a site only when no other form is among them.
 */
function unsafeSiblingForms(project, siblings, unsafe, tracked) {
  const forms = new Set();
  const consider = (name, value) => {
    if (name === tracked) return;
    if (value && !callableValue(project, value)) return;
    if (unsafe(name)) forms.add(siblingForm(project, siblings, name));
  };
  if (siblings.kind === "element") {
    for (const attribute of siblings.node.attributes.properties) {
      if (ts.isJsxAttribute(attribute) && attribute.initializer && ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression) consider(attribute.name.getText(), attribute.initializer.expression);
    }
  } else {
    for (const property of siblings.node.properties) {
      // The members a spread brings are not seen: any of them may be what the callee calls.
      if (ts.isSpreadAssignment(property)) forms.add("spread");
      const name = property.name ? staticName(property.name) : null;
      if (name === null || name === "<computed>") continue;
      consider(name, ts.isShorthandPropertyAssignment(property) ? property.name : ts.isPropertyAssignment(property) ? property.initializer : null);
    }
  }
  return [...forms].sort().join("+");
}

/** The form of the caller's value for `name` among `siblings` (an object literal or an element), or why there is none. */
function siblingForm(project, siblings, name) {
  if (siblings.kind === "element") {
    const properties = siblings.node.attributes.properties;
    const attribute = [...properties].reverse().find((property) => ts.isJsxAttribute(property) && property.name.getText() === name);
    if (!attribute) return properties.some((property) => ts.isJsxSpreadAttribute(property)) ? "element-spread" : "absent";
    if (!attribute.initializer || !ts.isJsxExpression(attribute.initializer) || !attribute.initializer.expression) return "literal";
    return valueForm(project, attribute.initializer.expression);
  }
  let found = null;
  for (const property of siblings.node.properties) if (property.name && staticName(property.name) === name) found = property;
  if (!found) return "absent";
  if (ts.isAccessor(found)) return "accessor";
  if (ts.isMethodDeclaration(found)) return "method";
  return valueForm(project, ts.isShorthandPropertyAssignment(found) ? found.name : found.initializer);
}

/** Whether `node` is an arrow function, or a `const` bound to one: a value that cannot use the object it is called on as `this`. */
function arrowValue(project, node) {
  const value = unwrap(node);
  if (ts.isArrowFunction(value)) return true;
  if (!ts.isIdentifier(value)) return false;
  const declaration = valueDeclaration(project, value);
  const list = declaration && ts.isVariableDeclaration(declaration) ? declaration.parent : null;
  return !!(list && ts.isVariableDeclarationList(list) && list.flags & ts.NodeFlags.Const && declaration.initializer && ts.isArrowFunction(unwrap(declaration.initializer)));
}

/** Whether the caller's value for the property `name` of `siblings` cannot use a `this` it is called with (sibling_ignores_receiver). */
function siblingSafe(project, siblings, name) {
  if (!siblings) return false;
  if (siblings.kind === "element") {
    const attribute = [...siblings.node.attributes.properties].reverse().find((property) => ts.isJsxAttribute(property) && property.name.getText() === name);
    if (!attribute) return false;
    if (!attribute.initializer || ts.isStringLiteral(attribute.initializer)) return true;
    return ts.isJsxExpression(attribute.initializer) && !!attribute.initializer.expression && arrowValue(project, attribute.initializer.expression);
  }
  let found = null;
  for (const property of siblings.node.properties) if (property.name && staticName(property.name) === name) found = property;
  if (!found || ts.isMethodDeclaration(found) || ts.isAccessor(found)) return false;
  const value = ts.isShorthandPropertyAssignment(found) ? found.name : found.initializer;
  return arrowValue(project, value) || ts.isStringLiteralLike(unwrap(value)) || ts.isNumericLiteral(unwrap(value));
}

/**
 * What the caller's side of a handed-in function keeps callee_use_site_role
 * from reading: an options object that is not closed (a getter, a spread, a
 * computed or repeated key) or holds a function that may use `this`, a
 * component element with a spread or a non-arrow function attribute.
 */
function callerSideBlocker(project, siblings) {
  if (!siblings) return null;
  if (siblings.kind === "element") {
    const properties = siblings.node.attributes.properties;
    if (properties.some((property) => ts.isJsxSpreadAttribute(property))) return "element-spread";
    for (const property of properties) {
      if (!ts.isJsxAttribute(property) || !property.initializer || !ts.isJsxExpression(property.initializer) || !property.initializer.expression) continue;
      const value = unwrap(property.initializer.expression);
      if (isFunction(value) && !ts.isArrowFunction(value)) return "element-function-attribute";
    }
    return null;
  }
  const names = new Set();
  for (const property of siblings.node.properties) {
    if (ts.isSpreadAssignment(property)) return "object-spread";
    if (ts.isAccessor(property)) return "object-getter";
    const name = property.name ? staticName(property.name) : null;
    if (name === "<computed>" || name === null) return "object-computed-key";
    if (names.has(name)) return "object-repeated-key";
    names.add(name);
  }
  for (const property of siblings.node.properties) {
    if (ts.isMethodDeclaration(property)) return "object-method";
    if (ts.isPropertyAssignment(property) && isFunction(unwrap(property.initializer)) && !ts.isArrowFunction(unwrap(property.initializer))) return "object-function-value";
  }
  return null;
}

/** The reason a use of a handed-in function (or one of its members) is not a followable call, or its position. */
function valueUseReason(project, callee, value, read, siblings, key, depth) {
  const { node, parent } = peel(value);
  if (!parent) return "stored";
  if (read) return invocationPosition(project, callee, value).replace(/^called-in-/, "read-in-");
  if (ts.isCallExpression(parent) && parent.expression === node) return invocationPosition(project, callee, parent);
  // Testing the value (`cb ? ... : ...`, `cb && ...`, `typeof cb`) runs nothing.
  if (ts.isTypeOfExpression(parent)) return null;
  if ((ts.isConditionalExpression(parent) && parent.condition === node) || (ts.isBinaryExpression(parent) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.QuestionQuestionToken].includes(parent.operatorToken.kind) && parent.left === node)) return null;
  const use = useOf(project, value);
  if (use.kind === "argument") return `handed-to-${originKind(use.origin)}`;
  if (use.kind === "intrinsic-handler") return "handed-to-intrinsic-handler";
  if (use.kind === "component-prop") {
    // Forwarded as exactly another project component's prop, which the proof places in turn.
    if (key === null || use.origin !== "project" || depth >= 3) return "forwarded-to-component";
    const element = use.element;
    const blocker = callerSideBlocker(project, { kind: "element", node: element });
    if (blocker) return `forwarded-through-${blocker}`;
    let component = use.declaration;
    if (component && ts.isVariableDeclaration(component)) component = component.initializer && isFunction(unwrap(component.initializer)) ? unwrap(component.initializer) : null;
    if (!component || !isFunction(component)) return "forwarded-to-non-function-component";
    const inner = calleeBlocker(project, component, 0, { key: use.name, siblings: { kind: "element", node: element }, depth: depth + 1 });
    return /^(called|read)-in-callbacks?$/.test(inner) ? null : `forwarded>${inner}`;
  }
  if (use.kind === "bound") return "aliased";
  if (use.kind === "destructured") return "destructured";
  return use.kind === "object-member" ? `stored:object-member@${objectFate(project, use.object)}` : `stored:${use.kind}`;
}

/**
 * What blocks the project function `callee` from vouching for how it uses its
 * `index`th parameter (callee_parameter_use_role's checks in its order: the
 * function and parameter shapes, then each use), or `called-in-callbacks` when
 * every use is a call in a followable callback. `key` is the member of the
 * parameter the handed-in function is (null: the parameter itself); `thunk`
 * says the parameter is a function returning that object; `siblings` is the
 * caller's options object or element, whose other members the callee may call.
 */
function calleeBlocker(project, callee, index, { key = null, thunk = false, read = false, siblings = null, depth = 0, defaulted = false } = {}) {
  if (hasModifier(callee, ts.SyntaxKind.AsyncKeyword) || callee.asteriskToken) return "async-callee";
  const parameter = callee.parameters[index];
  if (!parameter) return "no-such-parameter";
  if (!ts.isIdentifier(parameter.name)) return "destructured-parameter";
  // A default the proof does not follow, then what blocks the uses past it: following it moves the sites whose uses are followable.
  if (parameter.initializer && !defaulted) return `default-parameter/${calleeBlocker(project, callee, index, { key, thunk, read, siblings, depth, defaulted: true })}`;
  if (parameter.dotDotDotToken) return "rest-parameter";
  const symbol = project.symbol(parameter.name);
  const references = (symbol ? project.referencesOf(symbol) : []).filter((reference) => callee.body && callee.body.pos <= reference.pos && reference.end <= callee.body.end);
  if (!references.length) return "parameter-unused";
  if (references.some(isWrittenTo)) return "parameter-written";
  const reasons = [];
  const value = (node) => {
    const reason = valueUseReason(project, callee, node, read, siblings, key, depth);
    if (reason) reasons.push(reason);
  };
  // `node` denotes the parameter's object (or the function itself when `key` is null).
  const object = (node) => {
    const { node: self, parent } = peel(node);
    if (key === null) return value(node);
    if (parent && (ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) && parent.expression === self) {
      if (ts.isElementAccessExpression(parent)) return void reasons.push("computed-member");
      if (isWrittenTo(parent)) return void reasons.push("member-assigned");
      if (parent.name.text === key) return value(parent);
      // Another member: reading it runs nothing of the caller's; calling it, tagging with it or reaching through it hands the object over as `this`.
      const outer = peel(parent);
      const hands = outer.parent && (((ts.isCallExpression(outer.parent) || ts.isNewExpression(outer.parent)) && outer.parent.expression === outer.node) || (ts.isTaggedTemplateExpression(outer.parent) && outer.parent.tag === outer.node) || ((ts.isPropertyAccessExpression(outer.parent) || ts.isElementAccessExpression(outer.parent)) && outer.parent.expression === outer.node));
      if (hands && !siblingSafe(project, siblings, parent.name.text)) reasons.push(`sibling-called/${unsafeSiblingForms(project, siblings, (member) => !siblingSafe(project, siblings, member), key) || siblingForm(project, siblings, parent.name.text)}`);
      return;
    }
    // Testing the object (`if (options)`, `options ?? x`, `!options`) hands it nowhere.
    if (parent && (ts.isIfStatement(parent) || (ts.isConditionalExpression(parent) && parent.condition === self) || ts.isPrefixUnaryExpression(parent) || ts.isTypeOfExpression(parent))) return;
    if (parent && (ts.isSpreadAssignment(parent) || ts.isSpreadElement(parent))) return void reasons.push("object-spread");
    const use = useOf(project, node);
    reasons.push(use.kind === "argument" ? `object-handed-to-${originKind(use.origin)}` : `object-${use.kind}`);
  };
  for (const reference of references) {
    if (!thunk) {
      object(reference);
      continue;
    }
    const { node: self, parent } = peel(reference);
    if (parent && ts.isCallExpression(parent) && parent.expression === self) {
      // The object the thunk returns: whatever the callee makes of it. Its
      // binding's uses are followed one step (`const resolved = options()`).
      const result = peel(parent);
      if (result.parent && ts.isVariableDeclaration(result.parent) && ts.isIdentifier(result.parent.name)) for (const alias of project.referencesOf(project.symbol(result.parent.name) ?? { name: "" })) object(alias);
      else object(parent);
    } else {
      const use = useOf(project, reference);
      reasons.push(use.kind === "argument" ? `thunk-handed-to-${originKind(use.origin)}` : `thunk-${use.kind}`);
    }
  }
  if (!reasons.length) return "no-use";
  // The first use in source order that is not a followable call: the one the
  // callee's walk stops at.
  return reasons.find((reason) => !/^(called|read)-in-callback$/.test(reason)) ?? "called-in-callbacks";
}

/** The `/blocker` segment of a cause: none when `key` is undefined (a reading that does not refine), `nested-member` for a member of a nested object. */
function handedSuffix(project, call, node, key, options) {
  if (key === undefined || scope.rule !== "SC1001") return "";
  if (key === null) return "/nested-member";
  return `/${handedToProjectCause(project, call, node, { ...options, key })}`;
}

/**
 * Where a function handed to a project function at `call` goes: the callee's
 * blocker when the callee is a project function, else what the callee is.
 */
function handedToProjectCause(project, call, node, options) {
  const callee = calleeDeclaration(project, call);
  if (callee.kind !== "function") return `callee-${callee.kind}`;
  // The caller's side stops first: an options object the callee cannot be
  // trusted with. A getter is the thing read, not a callback, so it asks
  // what the callee makes of the object instead.
  const caller = options.read ? null : callerSideBlocker(project, options.siblings);
  return caller ?? calleeBlocker(project, callee.fn, argumentIndex(call, node), options);
}

/** What happens to an object literal: returned from a factory, exported, bound, passed. */
function objectFate(project, object, key = undefined, accessor = false, depth = 0) {
  const refine = key !== undefined;
  // A nested object (one reached through a local object) names its destination.
  const split = refine && depth > 0;
  const use = useOf(project, object);
  if (readSite && use.kind === "array-element") return `array-element:${arrayFate(project, peel(object).parent)}`;
  if (use.kind === "returned") {
    // An options object a thunk returns to its consumer (`useQuery(() => ({ queryFn }))`).
    const thunk = enclosingFunction(object);
    const handed = thunk && useOf(project, thunk);
    if (handed?.kind === "argument") {
      const where = originKind(handed.origin);
      return `options-thunk:${where}${where === "project" ? handedSuffix(project, handed.call, thunk, key, { thunk: true, read: accessor, siblings: { kind: "object", node: object } }) : ""}`;
    }
    return "factory-result";
  }
  if (use.kind === "bound") {
    const statement = use.declaration.parent?.parent;
    if (statement && hasModifier(statement, ts.SyntaxKind.ExportKeyword)) return "exported-object";
    const holder = enclosingFunction(object);
    const name = use.declaration.name;
    const references = project.referencesOf(project.symbol(name) ?? { name: "" });
    if (references.some((reference) => ["returned", "exported"].includes(useOf(project, reference).kind))) return "factory-result";
    const propUse = references.map((reference) => useOf(project, reference)).find((reference) => reference.kind === "component-prop");
    if (propUse) return readSite ? propValueFate(project, propUse) : split ? componentPropFate(project, propUse) : "context-or-prop-value";
    if (!refine) return holder ? "local-object" : "module-object";
    // Where the object goes besides its own members' reads: handed to a package
    // function, put in another object, assigned to a variable, spread.
    const fates = new Set();
    for (const reference of references) {
      const parent = reference.parent;
      if (ts.isPropertyAccessExpression(parent) && parent.expression === reference) continue;
      const referenced = useOf(project, reference);
      if (referenced.kind === "bound") continue;
      if (referenced.kind === "argument") fates.add(`argument-of:${originKind(referenced.origin)}`);
      else if (referenced.kind === "object-member" && depth < 3) fates.add(objectFate(project, referenced.object, null, accessor, depth + 1));
      else fates.add(referenced.kind);
    }
    if (holder) return fates.size ? `local-object:${[...fates].sort().join("|")}` : "local-object";
    return "module-object";
  }
  if (use.kind === "component-prop") return readSite ? propValueFate(project, use) : split ? componentPropFate(project, use) : "context-or-prop-value";
  if (use.kind === "argument") {
    const where = originKind(use.origin);
    return `argument-of:${where}${where === "project" ? handedSuffix(project, use.call, object, key, { read: accessor, siblings: { kind: "object", node: object } }) : ""}`;
  }
  if (use.kind === "object-member") return objectFate(project, use.object, key === undefined ? undefined : null, accessor, depth + 1);
  return use.kind;
}

/**
 * An object written in a component's attribute: a context's `value`
 * (`createContext`'s result used as the tag) or a project component's prop.
 */
function componentPropFate(project, use) {
  const declaration = use.declaration;
  if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer && ts.isCallExpression(declaration.initializer)) {
    const callee = project.callee(declaration.initializer);
    if (callee.declared === "createContext" && isSolidOrgPackage(callee.origin)) return "context-value";
  }
  return use.origin === "project" ? "component-prop-value" : "package-component-prop-value";
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
export function componentCensus(project, fn) {
  const uses = usesOf(project, fn);
  const tags = [];
  const other = [];
  for (const use of uses) {
    if (use.kind === "JsxOpeningElement" || use.kind === "JsxSelfClosingElement" || use.kind === "JsxClosingElement") tags.push(use);
    else if (!["ImportSpecifier", "ImportClause", "exported", "ExportSpecifier", "TypeQuery"].includes(use.kind)) other.push(use);
  }
  return { tags, other, exported: uses.some((use) => use.kind === "exported") };
}

export function jsxTagSpreads(use) {
  const element = use.node?.parent;
  const attributes = element?.attributes?.properties ?? [];
  return attributes.some((attribute) => ts.isJsxSpreadAttribute(attribute));
}

/**
 * The placement cause of a site held by function `fn` (writes and reads
 * alike). A helper that is only called takes the cause of its callers'
 * positions (`via-callers:<cause>`), followed `depth` calls up.
 */
function placementCause(project, fn, site, depth = 0, seen = new Set(), refine = true) {
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
    // A read's cause says what the callee makes of the object (callee_parameter_use_role); a write's is followed by the write walk.
    const reading = refine && scope.rule === "SC1001";
    if (object) return `object-member:${objectFate(project, object.object, reading ? object.key : undefined)}`;
    const accessor = reading && uses.find((use) => use.kind === "object-accessor");
    if (accessor) return `${accessor.accessor}-accessor:${objectFate(project, accessor.object, accessor.key, true)}`;
    if (kinds.has("component-prop")) {
      const prop = uses.find((use) => use.kind === "component-prop");
      if (prop.origin !== "project") return "package-component-prop";
      const tail = childInvocationCause(project, prop.declaration, prop.name, depth, seen);
      // A callback in a read: what stops the callee's proof of the prop it is handed.
      if (refine && scope.rule === "SC1001" && scope.family === "callback-to-unproven-consumer" && PROP_TAILS.has(tail)) return `component-prop>${tail}/${propBlocker(project, prop)}`;
      return `component-prop>${tail}`;
    }
    if (kinds.has("intrinsic-handler")) return "intrinsic-handler";
    const argument = uses.find((use) => use.kind === "argument");
    if (argument) {
      // A callback handed to a lib.dom function (a timer, an observer) after
      // its caller may have suspended runs on that continuation's stack.
      const caller = enclosingFunction(argument.call);
      if (argument.origin === "lib-dom" && caller && afterSuspension(caller, argument.call)) return "after-await";
      if (["addEventListener", "removeEventListener"].includes(argument.name) && argument.origin.startsWith("lib")) return "listener-callback";
      if (argument.origin === "project") return refine && scope.rule === "SC1001" ? `project-function-argument/${handedToProjectCause(project, argument.call, argument.node ?? current, {})}` : "project-function-argument";
      if (argument.origin.startsWith("package")) return "package-callback";
      return `${argument.origin}-callback`;
    }
    if (kinds.has("called")) {
      if (depth >= (refine && scope.rule === "SC2001" ? 6 : 3) || seen.has(current)) return "helper-call-chain";
      seen.add(current);
      const writing = refine && scope.rule === "SC2001";
      // Each caller's own cause: for a write, the structure the caller sits in
      // and where the walk of its invocations stops (`full`).
      const callers = new Map();
      for (const use of uses.filter((one) => one.kind === "called")) {
        const caller = enclosingFunction(use.call);
        const cause = placementCause(project, caller, use.call, depth + 1, seen, refine).replace(/^via-callers:/, "");
        if (writing && cause.startsWith("mixed:")) {
          for (const part of cause.slice("mixed:".length).split("|")) callers.set(part, part);
        } else callers.set(cause + (writing && scope.family === "unplaced-write" && WALKED_STRUCTURES.has(cause) ? `/${writeWalkReason(project, walkRoot(project, caller))}` : ""), cause);
      }
      if (kinds.has("exported") && !uses.some((use) => use.kind !== "called" && use.kind !== "exported")) callers.set("exported", "exported");
      const ordered = [...callers.keys()].sort();
      // A caller that is a delivery (an intrinsic handler) places its own entry, so
      // the entries that keep the write unplaced are the others.
      const open = writing && ordered.length > 1 ? ordered.filter((cause) => cause !== "intrinsic-handler") : ordered;
      if (open.length === 1) return `via-callers:${VIA_WALKED_STRUCTURES.has(callers.get(open[0])) ? open[0] : callers.get(open[0]) ?? open[0]}`;
      return `via-callers:${ordered.includes("after-await") ? "after-await" : writing ? `mixed:${open.join("|")}` : "mixed"}`;
    }
    if (kinds.has("returned")) {
      // Who receives the closure depends on what the function returning it is.
      return "returned-closure";
    }
    if (kinds.has("unreferenced") || (kinds.size === 1 && kinds.has("exported"))) return "unreferenced-function";
    return `other:${[...kinds][0] === "object-accessor" ? "ObjectLiteralExpression" : [...kinds][0]}`;
  }
  return "deep-chain";
}

// ---------------------------------------------------------------------------
// The write walk: why the checker's forward walk of a function's invocations
// (ADR 0319, 0442; `InvocationFlow::uses` in execution_role/event_literals.rs)
// stops. The walk starts from the function that holds a write, follows the
// value through the holders it models (calls, member accesses, arguments of
// project functions, component props, object members, returns, `const`
// bindings, conditional arms) and returns the first position it does not
// model. Whether the checker starts a walk at the function at all is not
// asked: the reason is where the walk would stop if it did, so that one
// change (modelling that position) moves the sites that share a reason.

const WALK_BUDGET = 800;
const FRESH_STACK_CALLBACKS = new Set(["setTimeout", "setInterval", "queueMicrotask", "requestAnimationFrame", "requestIdleCallback"]);
/** `this` or `super` as a word, in text or comments too, as the checker reads it (mentions_this). */
const THIS_WORD = /(?<![\w$.])(this|super)(?![\w$])/;

/**
 * Whether a function written in `node` that is not an arrow function may use
 * `this` or `super`: "real" when one is written as code, "text" when only a
 * comment or string mentions the word (the checker reads the text).
 */
function receiverUsedWithin(node) {
  let found = null;
  const real = (fn) => {
    let any = false;
    const visit = (child) => {
      if (any) return;
      if (child.kind === ts.SyntaxKind.ThisKeyword || child.kind === ts.SyntaxKind.SuperKeyword) any = true;
      else ts.forEachChild(child, visit);
    };
    visit(fn);
    return any;
  };
  const visit = (child) => {
    if (found === "real") return;
    if (isFunction(child) && !ts.isArrowFunction(child) && THIS_WORD.test(child.getText())) found = real(child) ? "real" : (found ?? "text");
    ts.forEachChild(child, visit);
  };
  visit(node);
  return found;
}

const thisReason = (kind) => (kind === "real" ? "object-uses-this" : "object-mentions-this-in-text");

/** Whether `node` is written in a type (`typeof f` in a type, a type argument): no value flows through it. */
function inTypePosition(node) {
  for (let parent = node.parent; parent; parent = parent.parent) {
    if (ts.isTypeNode(parent) && !ts.isExpressionWithTypeArguments(parent)) return true;
    if (ts.isStatement(parent) || isFunction(parent)) return false;
  }
  return false;
}

const isIntrinsicTag = (tag) => ts.isIdentifier(tag) && /^[a-z]/.test(tag.text);

/** Whether `node` is written to: an assignment, `++`, `--` or a for-in/of target. */
function isWrittenTo(node) {
  const { node: self, parent } = peel(node);
  if (!parent) return false;
  if (ts.isBinaryExpression(parent) && parent.left === self) {
    const kind = parent.operatorToken.kind;
    return kind >= ts.SyntaxKind.FirstAssignment && kind <= ts.SyntaxKind.LastAssignment;
  }
  if ((ts.isPrefixUnaryExpression(parent) || ts.isPostfixUnaryExpression(parent)) && [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(parent.operator)) return true;
  if ((ts.isForInStatement(parent) || ts.isForOfStatement(parent)) && parent.initializer === self) return true;
  if (ts.isArrayLiteralExpression(parent) || ts.isSpreadElement(parent) || ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent)) {
    // A destructuring assignment target (`[a, b] = ...`, `({ a } = ...)`).
    let outer = parent;
    while (outer.parent && (ts.isArrayLiteralExpression(outer.parent) || ts.isObjectLiteralExpression(outer.parent) || ts.isPropertyAssignment(outer.parent) || ts.isShorthandPropertyAssignment(outer.parent) || ts.isSpreadElement(outer.parent))) outer = outer.parent;
    return !!outer.parent && ts.isBinaryExpression(outer.parent) && outer.parent.left === outer && outer.parent.operatorToken.kind === ts.SyntaxKind.EqualsToken;
  }
  return false;
}

class WriteWalk {
  constructor(project) {
    this.project = project;
    this.budget = WALK_BUDGET;
    this.seen = new Set();
    this.propsStack = [];
    this.invocations = [];
  }

  /** The reference identifiers of the binding `name` (not the declaration). */
  references(name) {
    const symbol = this.project.symbol(name);
    return (symbol ? this.project.referencesOf(symbol) : []).filter((reference) => !inTypePosition(reference) && !ts.isImportSpecifier(reference.parent) && !ts.isImportClause(reference.parent));
  }

  /** Whether the binding named `name` is assigned anywhere. */
  written(name) {
    return this.references(name).some(isWrittenTo);
  }

  /** Whether a module-level binding is exported (a modifier or an export specifier of its name). */
  exported(name, declaration) {
    const statement = ts.isVariableDeclaration(declaration) ? declaration.parent?.parent : declaration;
    if (statement && hasModifier(statement, ts.SyntaxKind.ExportKeyword)) return true;
    return this.references(name).some((reference) => ts.isExportSpecifier(reference.parent) || ts.isExportAssignment(reference.parent));
  }

  /** The reason the walk of every invocation of `fn` stops, or null; the invocations it found are in `this.invocations`. */
  functionInvocations(fn) {
    if (!ts.isExternalModule(fn.getSourceFile())) return "script-file";
    const outer = this.seen;
    this.seen = new Set();
    try {
      if ((ts.isMethodDeclaration(fn) || ts.isGetAccessorDeclaration(fn) || ts.isSetAccessorDeclaration(fn)) && ts.isObjectLiteralExpression(fn.parent)) {
        const key = staticName(fn.name);
        if (key === "<computed>") return "computed-key";
        const receiver = receiverUsedWithin(fn.parent);
        if (receiver) return thisReason(receiver);
        return this.uses(fn.parent, [key], { kind: "object", node: fn.parent });
      }
      if (ts.isMethodDeclaration(fn) || ts.isConstructorDeclaration(fn) || ts.isGetAccessorDeclaration(fn) || ts.isSetAccessorDeclaration(fn)) return "class-member";
      if (ts.isFunctionDeclaration(fn)) {
        if (!fn.name) return "anonymous-declaration";
        if (this.written(fn.name)) return "function-name-written";
        const exported = this.exported(fn.name, fn);
        for (const reference of this.references(fn.name)) {
          if (exported && reference.getSourceFile() !== fn.getSourceFile() && !(peel(reference).parent && ts.isCallExpression(peel(reference).parent) && peel(reference).parent.expression === peel(reference).node)) return `exported-reference:${useOf(this.project, reference).kind}`;
          const reason = this.uses(reference, [], { kind: "unknown" });
          if (reason) return reason;
        }
        return null;
      }
      if (ts.isFunctionExpression(fn) && fn.name) {
        // A named function expression can hand itself on by its own name.
        const own = this.references(fn.name);
        if (own.length) return "named-expression-self-reference";
      }
      return this.uses(fn, [], { kind: "unknown" });
    } finally {
      this.seen = outer;
    }
  }

  /** The reason following the value of `expr`, held at member path `path`, stops. */
  uses(expr, path, origin) {
    if (this.budget === 0) return "walk-budget";
    if (!ts.isExternalModule(expr.getSourceFile())) return "script-file";
    this.budget--;
    const marker = `${expr.getSourceFile().fileName}:${expr.pos}:${expr.end}|${path.join(".")}|${this.propsStack.map((frame) => frame.element.pos).join(",")}`;
    if (this.seen.has(marker)) return null;
    this.seen.add(marker);
    const project = this.project;
    const { node: self, parent } = peel(expr);
    if (!parent) return "unmodeled:none";
    if (ts.isImportSpecifier(parent) || ts.isImportClause(parent)) return null;
    if (ts.isExportSpecifier(parent) || ts.isExportAssignment(parent)) return "handed-out:export";
    // Called.
    if ((ts.isCallExpression(parent) || ts.isNewExpression(parent)) && parent.expression === self) {
      if (ts.isNewExpression(parent)) return `constructed:${originKind(project.originOf(parent.expression))}`;
      if (path.length) return "called-with-member-path";
      this.invocations.push({ call: parent });
      return null;
    }
    if (ts.isTaggedTemplateExpression(parent) && parent.tag === self) return "tagged-template";
    // The object of a member access.
    if ((ts.isPropertyAccessExpression(parent) || ts.isElementAccessExpression(parent)) && parent.expression === self) {
      if (isWrittenTo(parent)) return "member-assigned";
      // A component tag taken from the props (`<props.row.icon />`): an experiment on the checker
      // shows the same function certified without it (the walk's own reason is not read off the code).
      let top = parent;
      while (top.parent && ts.isPropertyAccessExpression(top.parent) && top.parent.expression === top) top = top.parent;
      if (origin.kind === "element" && (ts.isJsxOpeningElement(top.parent) || ts.isJsxSelfClosingElement(top.parent)) && top.parent.tagName === top) return "props-member-as-jsx-tag";
      const key = ts.isPropertyAccessExpression(parent) ? parent.name.text : ts.isStringLiteralLike(parent.argumentExpression) || ts.isNumericLiteral(parent.argumentExpression) ? parent.argumentExpression.text : null;
      if (key === null) return "computed-member";
      const outer = peel(parent);
      const called = outer.parent && (((ts.isCallExpression(outer.parent) || ts.isNewExpression(outer.parent)) && outer.parent.expression === outer.node) || (ts.isTaggedTemplateExpression(outer.parent) && outer.parent.tag === outer.node));
      if (!path.length) return called ? "function-property-called" : null;
      if (path[0] === key) return this.uses(parent, path.slice(1), { kind: "unknown" });
      if (called) return this.originMemberIgnoresReceiver(origin, key, 6) ? null : `sibling-called${origin.kind === "element" ? ":prop" : ""}/${this.originMemberForm(origin, key, path[0])}`;
      return null;
    }
    // An argument.
    if ((ts.isCallExpression(parent) || ts.isNewExpression(parent)) && parent.arguments?.includes(self)) return this.argumentUse(parent, self, path, origin);
    if (ts.isSpreadElement(parent) && parent.parent && (ts.isCallExpression(parent.parent) || ts.isNewExpression(parent.parent))) return "argument-spread";
    // A JSX attribute value or spread, or a child.
    if (ts.isJsxExpression(parent) && parent.parent && ts.isJsxAttribute(parent.parent)) {
      const attribute = parent.parent;
      const element = attribute.parent.parent;
      const name = attribute.name.getText();
      if (name.includes(":")) return "namespaced-attribute";
      if (isIntrinsicTag(element.tagName)) {
        if (!path.length && /^on[A-Za-z]/.test(name)) {
          this.invocations.push({ role: "event" });
          return null;
        }
        return `intrinsic-attribute:${/^on/.test(name) ? "event-through-path" : "not-event"}`;
      }
      return this.componentPropsUses(element, [name, ...path], this.enclosingProps(element));
    }
    if (ts.isJsxSpreadAttribute(parent)) {
      const element = parent.parent.parent;
      if (isIntrinsicTag(element.tagName)) {
        if (!path.length) return null;
        if (path.length === 1 && /^on[A-Z]/.test(path[0])) {
          this.invocations.push({ role: "event" });
          return null;
        }
        return "spread-onto-intrinsic:not-event";
      }
      return "spread-onto-component";
    }
    if (ts.isJsxExpression(parent) && parent.parent && (ts.isJsxElement(parent.parent) || ts.isJsxFragment(parent.parent))) {
      const element = ts.isJsxElement(parent.parent) ? parent.parent.openingElement : null;
      if (!element || isIntrinsicTag(element.tagName)) return "jsx-child-of-intrinsic";
      const children = parent.parent.children.filter((child) => !(ts.isJsxText(child) && child.containsOnlyTriviaWhiteSpaces));
      if (children.length !== 1 || element.attributes.properties.some((p) => ts.isJsxAttribute(p) && p.name.getText() === "children")) return "children-prop-competes";
      return this.componentPropsUses(element, ["children", ...path], this.enclosingProps(element));
    }
    // A property value of an object literal.
    if ((ts.isPropertyAssignment(parent) && parent.initializer === self) || ts.isShorthandPropertyAssignment(parent)) {
      const object = parent.parent;
      const key = staticName(parent.name);
      if (key === "<computed>") return "computed-key";
      if (object.properties.some((candidate) => candidate !== parent && candidate.name && staticName(candidate.name) === key && ts.isAccessor(candidate))) return "object-getter";
      const receiver = receiverUsedWithin(object);
      if (receiver) return thisReason(receiver);
      return this.uses(object, [key, ...path], { kind: "object", node: object });
    }
    // Spread into an object literal.
    if (ts.isSpreadAssignment(parent)) {
      const object = parent.parent;
      if (!path.length) return null;
      const later = object.properties.slice(object.properties.indexOf(parent) + 1);
      if (later.some((candidate) => candidate.name && staticName(candidate.name) === path[0])) return null;
      if (object.properties.some((candidate) => candidate.name && staticName(candidate.name) === path[0] && ts.isAccessor(candidate))) return "object-getter";
      const receiver = receiverUsedWithin(object);
      if (receiver) return thisReason(receiver);
      return this.uses(object, path, { kind: "object", node: object });
    }
    // Returned.
    if (ts.isReturnStatement(parent) || (ts.isArrowFunction(parent) && parent.body === self)) {
      const returner = ts.isReturnStatement(parent) ? enclosingFunction(parent) : parent;
      if (!returner) return "unmodeled:return-outside-function";
      if (hasModifier(returner, ts.SyntaxKind.AsyncKeyword) || returner.asteriskToken) return "returned-from-async";
      const saved = this.invocations;
      this.invocations = [];
      const reason = this.functionInvocations(returner);
      const calls = this.invocations;
      this.invocations = saved;
      if (reason) return reason;
      for (const invocation of calls) {
        if (!invocation.call) continue;
        const outcome = this.uses(invocation.call, path, origin);
        if (outcome) return outcome;
      }
      return null;
    }
    // Bound to a `const`.
    if (ts.isVariableDeclaration(parent) && parent.initializer === self) {
      if (ts.isObjectBindingPattern(parent.name)) {
        const elements = parent.name.elements;
        if (elements.some((element) => element.dotDotDotToken || !ts.isIdentifier(element.name) || (element.propertyName && !ts.isIdentifier(element.propertyName)))) return "destructured-complex";
        if (!path.length) return "destructured-function";
        const slot = elements.find((element) => (element.propertyName ?? element.name).text === path[0]);
        if (!slot) return null;
        if (this.written(slot.name) || this.exported(slot.name, parent)) return "binding-written-or-exported";
        for (const reference of this.references(slot.name)) {
          const reason = this.uses(reference, path.slice(1), { kind: "unknown" });
          if (reason) return reason;
        }
        return null;
      }
      if (!ts.isIdentifier(parent.name)) return "destructured-array";
      const list = parent.parent;
      const isConst = list && ts.isVariableDeclarationList(list) && (list.flags & ts.NodeFlags.Const) !== 0;
      if (!isConst && this.written(parent.name)) return "binding-written-or-exported";
      if (this.exported(parent.name, parent)) return "binding-written-or-exported";
      for (const reference of this.references(parent.name)) {
        const reason = this.uses(reference, path, origin);
        if (reason) return reason;
      }
      return null;
    }
    // A conditional's arm, or its test; a logical operand.
    if (ts.isConditionalExpression(parent)) return parent.condition === self ? null : this.uses(parent, path, { kind: "unknown" });
    if (ts.isBinaryExpression(parent) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(parent.operatorToken.kind)) {
      if (parent.left === self && parent.operatorToken.kind === ts.SyntaxKind.AmpersandAmpersandToken) return null;
      return this.uses(parent, path, { kind: "unknown" });
    }
    if (ts.isTypeOfExpression(parent) || ts.isPrefixUnaryExpression(parent) && parent.operator === ts.SyntaxKind.ExclamationToken) return null;
    if (ts.isBinaryExpression(parent) && parent.operatorToken.kind === ts.SyntaxKind.EqualsToken && parent.right === self) return "assigned-to-variable";
    return `unmodeled:${ts.SyntaxKind[parent.kind]}`;
  }

  /** The value `self` handed as an argument of `call`. */
  argumentUse(call, self, path, origin) {
    if (ts.isNewExpression(call)) return `constructed:${originKind(this.project.originOf(call.expression))}`;
    const project = this.project;
    const index = call.arguments.indexOf(self);
    if (call.arguments.slice(0, index + 1).some((argument) => ts.isSpreadElement(argument))) return "argument-spread";
    const callee = project.callee(call);
    if (isSolidOrgPackage(callee.origin)) return `handed-to-solid:${callee.declared ?? callee.name}`;
    if (!path.length && callee.origin.startsWith("lib") && FRESH_STACK_CALLBACKS.has(callee.declared ?? callee.name)) {
      this.invocations.push({ role: "deferred" });
      return null;
    }
    // A package's function is read from its source, or not at all: which package does not matter.
    if (callee.origin.startsWith("package")) {
      this.packageCallee = callee.declared ?? callee.name;
      return "handed-to-package";
    }
    if (callee.origin === "unresolved") return "handed-to-unresolved-callee";
    if (callee.origin !== "project") return `handed-to-${originKind(callee.origin)}:${callee.declared ?? callee.name}`;
    const declaration = calleeDeclaration(project, call);
    if (declaration.kind !== "function") return `handed-to-callee-${declaration.kind}`;
    if (isComponentLike(declaration.fn)) return "handed-to-component-function";
    return this.parameterUses(declaration.fn, index, path, origin);
  }

  /** The reason the walk of the uses of `callee`'s parameter `index` stops. */
  parameterUses(callee, index, path, origin, defaulted = false) {
    if (!ts.isExternalModule(callee.getSourceFile())) return "script-file";
    const parameter = callee.parameters[index];
    if (!parameter) return "no-such-parameter";
    if (!ts.isIdentifier(parameter.name)) return "destructured-parameter";
    // A default the walk does not follow, then where the walk past it stops.
    if (parameter.initializer && !defaulted) return `default-parameter/${this.parameterUses(callee, index, path, origin, true) ?? "no-other-blocker"}`;
    if (parameter.dotDotDotToken) return "rest-parameter";
    if (this.written(parameter.name)) return "parameter-written";
    if (/\b(arguments|eval)\b/.test(callee.getText())) return "arguments-or-eval";
    for (const reference of this.references(parameter.name)) {
      if (!callee.body || reference.pos < callee.body.pos || reference.end > callee.body.end) return "parameter-referenced-outside-body";
      const reason = this.uses(reference, path, origin);
      if (reason) return reason;
    }
    return null;
  }

  /** The props of the component activation the walk entered, for an element written in it. */
  enclosingProps(element) {
    for (let i = this.propsStack.length - 1; i >= 0; i--) {
      const frame = this.propsStack[i];
      if (frame.component.pos <= element.pos && element.end <= frame.component.end) return frame.origin;
    }
    return { kind: "unknown" };
  }

  /** The reason following `path` into the props of the component `element` renders stops. */
  componentPropsUses(element, path, enclosing) {
    const project = this.project;
    const tag = element.tagName;
    const target = ts.isPropertyAccessExpression(tag) ? tag.name : tag;
    const declaration = project.symbol(target)?.declarations?.[0];
    let component = null;
    if (declaration && isFunction(declaration) && !ts.isMethodDeclaration(declaration)) component = declaration;
    else if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer && isFunction(unwrap(declaration.initializer))) component = unwrap(declaration.initializer);
    else if (declaration && ts.isMethodDeclaration(declaration)) component = declaration;
    if (!component) return `component-not-a-function:${declaration ? ts.isVariableDeclaration(declaration) && declaration.initializer && ts.isCallExpression(unwrap(declaration.initializer)) ? (isSolidOrgPackage(project.callee(unwrap(declaration.initializer)).origin) ? "solid-call" : project.callee(unwrap(declaration.initializer)).origin === "project" ? "project-factory" : "package-call") : ts.SyntaxKind[declaration.kind] : "unresolved"}`;
    if (project.originOf(target) !== "project") return "package-component";
    if (hasModifier(component, ts.SyntaxKind.AsyncKeyword) || component.asteriskToken) return "component-async";
    const props = element.attributes.properties;
    if (props.some((property) => ts.isJsxSpreadAttribute(property))) return "element-spread";
    const names = props.filter(ts.isJsxAttribute).map((attribute) => attribute.name.getText());
    if (path.length && names.filter((name) => name === path[0]).length > 1) return "element-repeated-attribute";
    if (ts.isPropertyAccessExpression(tag) && !ts.isMethodDeclaration(component) && !(ts.isArrowFunction(component) || ts.isFunctionExpression(component))) return "member-tag";
    const cycle = this.propsStack.findIndex((frame) => frame.element === element && frame.followed === path.join("."));
    if (cycle >= 0) return this.propsStack.slice(cycle).every((frame) => this.cycleForwards(frame)) ? null : "recursive-component";
    const origin = { kind: "element", element, enclosing };
    this.propsStack.push({ component, element, followed: path.join("."), origin });
    try {
      return this.parameterUses(component, 0, path, origin);
    } finally {
      this.propsStack.pop();
    }
  }

  /** A rendering cycle's element forwards each prop unchanged and has no spread. */
  cycleForwards(frame) {
    const props = frame.element.attributes.properties;
    return !props.some((property) => ts.isJsxSpreadAttribute(property));
  }

  /** Whether the member `key` of `origin` cannot use the object it is called on as `this`. */
  originMemberIgnoresReceiver(origin, key, depth) {
    if (depth === 0) return false;
    if (origin.kind === "object") {
      const properties = origin.node.properties;
      let index = -1;
      for (let i = 0; i < properties.length; i++) if (properties[i].name && staticName(properties[i].name) === key) index = i;
      if (index < 0) return false;
      const property = properties[index];
      if (properties.slice(index + 1).some((later) => ts.isSpreadAssignment(later) || (later.name && staticName(later.name) === "<computed>"))) return false;
      if (ts.isAccessor(property)) return false;
      if (ts.isMethodDeclaration(property)) return !THIS_WORD.test(property.getText());
      if (ts.isShorthandPropertyAssignment(property)) return this.valueIgnoresReceiver(property.name, { kind: "unknown" }, depth);
      return ts.isPropertyAssignment(property) ? this.valueIgnoresReceiver(property.initializer, { kind: "unknown" }, depth) : false;
    }
    if (origin.kind === "element") {
      const attributes = origin.element.attributes.properties;
      if (attributes.some((property) => ts.isJsxSpreadAttribute(property))) return false;
      const attribute = [...attributes].reverse().find((property) => ts.isJsxAttribute(property) && property.name.getText() === key);
      if (!attribute) return true;
      if (!attribute.initializer) return true;
      if (ts.isStringLiteral(attribute.initializer)) return true;
      if (ts.isJsxExpression(attribute.initializer) && attribute.initializer.expression) return this.valueIgnoresReceiver(attribute.initializer.expression, origin.enclosing, depth);
      return false;
    }
    return false;
  }

  /** The forms of the values `origin` gives for the members a callee could call besides `tracked`, `key` being the one called (see unsafeSiblingForms). */
  originMemberForm(origin, key, tracked) {
    if (origin.kind === "object") {
      const properties = origin.node.properties;
      let index = -1;
      for (let i = 0; i < properties.length; i++) if (properties[i].name && staticName(properties[i].name) === key) index = i;
      if (index >= 0 && properties.slice(index + 1).some((later) => ts.isSpreadAssignment(later) || (later.name && staticName(later.name) === "<computed>"))) return "later-spread";
    }
    if (origin.kind !== "object" && origin.kind !== "element") return "unknown-origin";
    const siblings = origin.kind === "object" ? { kind: "object", node: origin.node } : { kind: "element", node: origin.element };
    return unsafeSiblingForms(this.project, siblings, (member) => !this.originMemberIgnoresReceiver(origin, member, 6), tracked) || siblingForm(this.project, siblings, key);
  }

  /** Whether the value written at `node` cannot use a `this` it is called with. */
  valueIgnoresReceiver(node, enclosing, depth) {
    const value = unwrap(node);
    if (isFunction(value)) return ts.isArrowFunction(value) || !THIS_WORD.test(value.getText());
    if (ts.isIdentifier(value)) {
      const declaration = valueDeclaration(this.project, value);
      if (declaration && ts.isFunctionDeclaration(declaration)) return !THIS_WORD.test(declaration.getText());
      if (declaration && ts.isVariableDeclaration(declaration) && declaration.initializer) {
        const list = declaration.parent;
        const immutable = list && ts.isVariableDeclarationList(list) && (list.flags & ts.NodeFlags.Const) !== 0;
        const initializer = unwrap(declaration.initializer);
        return !!immutable && isFunction(initializer) && (ts.isArrowFunction(initializer) || !THIS_WORD.test(initializer.getText()));
      }
      return false;
    }
    // `props.x` read in the component the enclosing element renders: that element's attribute `x`.
    if (ts.isPropertyAccessExpression(value) && enclosing.kind === "element" && depth > 0) return this.originMemberIgnoresReceiver(enclosing, value.name.text, depth - 1);
    return ts.isArrayLiteralExpression(value) || ts.isStringLiteralLike(value) || ts.isNumericLiteral(value);
  }
}

/**
 * The structures whose writes the walk refines: the ones the reproduction
 * set's cases stand for (a structure no case reads is left as it was).
 */
const WALKED_STRUCTURES = new Set([
  "object-member:factory-result",
  "object-member:argument-of:project",
  "object-member:argument-of:package",
  "object-member:local-object",
  "object-member:context-or-prop-value",
  "object-member:new-argument",
  "project-function-argument",
  "other:ObjectLiteralExpression",
  "returned-closure",
  "component-prop>prop-unused",
  "component-prop>listener-callback",
  "component-prop>view:placed",
  "component-prop>placed",
  "component-prop>view+forwarded:view+forwarded:placed",
  "component-prop>forwarded:prop-unused",
  "component-prop>forwarded:deep-chain",
  "component-prop>component-prop>deep-chain",
  "component-prop>component-not-a-function",
  "component-prop>forwarded:placed",
  "component-prop>view:returned",
  "component-prop>returned",
  "component-prop>component-prop",
  "component-prop>view:component-prop",
  "component-prop>view:prop-unused",
]);

/** The structures a single caller's walk refines in a `via-callers` cause (the others have no case). */
const VIA_WALKED_STRUCTURES = new Set(["object-member:factory-result", "object-member:argument-of:project", "project-function-argument", "component-prop>prop-unused"]);

/** The structures whose `unplaced-call` results the walk refines (a call of a function, not a write). */
const WALKED_CALL_STRUCTURES = new Set(["object-member:factory-result", "component-prop>after-await"]);

/** The structures a read in a condition refines the same way as one outside it: the other condition variants have no case. */
const CONDITION_STRUCTURES = new Set(["project-function-argument", "other:ObjectLiteralExpression", "object-member:local-object"]);

/** The function the write walk starts from: the function holding the write, or the one around it when a standard-library callback runs it in place. */
function walkRoot(project, fn) {
  let root = fn;
  for (let level = 0; root && level < 6; level++) {
    const uses = usesOf(project, root);
    if (uses.length === 1 && isSynchronousLibraryCallback(uses[0])) root = enclosingFunction(root);
    else break;
  }
  return root ?? fn;
}

/**
 * Where the walk of the invocations of `fn` stops for a write: the walk's own
 * reason, else `unplaced-invocation:<cause>` for the first invocation that is
 * no delivery, else `walk-completes` (the walk finds every invocation and each
 * is a delivery, so the gap is elsewhere).
 */
function writeWalkReason(project, fn, structure = "") {
  const walk = new WriteWalk(project);
  const reason = walk.functionInvocations(fn);
  // A handler object handed to a package function: which function reads it from its source is what decides the entries.
  if (reason === "handed-to-package" && structure === "object-member:argument-of:package" && walk.packageCallee) return `handed-to-package:${walk.packageCallee}`;
  // A default the walk did not follow, with nothing else in its way: the invocations past it are read as they would be.
  const defaulted = reason === "default-parameter/no-other-blocker";
  if (reason && !defaulted) return reason;
  const invocationReason = () => {
    for (const invocation of walk.invocations) {
      if (invocation.role) continue;
      const cause = placementCause(project, enclosingFunction(invocation.call), invocation.call, 1, new Set(), false);
      if (cause !== "intrinsic-handler") return `unplaced-invocation:${cause.replace(/^via-callers:/, "")}`;
    }
    return walk.invocations.length ? "walk-completes" : "no-invocation";
  };
  return defaulted ? `default-parameter/${invocationReason()}` : invocationReason();
}

/** The tails of a component prop's cause that a read's cause refines with what stops the callee's proof. */
const PROP_TAILS = new Set(["prop-unused", "view:placed", "forwarded:prop-unused", "forwarded:deep-chain", "component-prop"]);

/** What stops the proof of the callback prop `prop`, handed to a project component: the element's own shape, else the component's use of the prop. */
function propBlocker(project, prop) {
  const siblings = { kind: "element", node: prop.element };
  const caller = callerSideBlocker(project, siblings);
  if (caller) return caller;
  let component = prop.declaration;
  if (component && ts.isVariableDeclaration(component)) component = component.initializer && isFunction(unwrap(component.initializer)) ? unwrap(component.initializer) : null;
  if (!component || !isFunction(component)) return "component-not-a-function";
  return calleeBlocker(project, component, 0, { key: prop.name, siblings });
}

const VIEW_CALLS = new Set(["omit", "merge", "splitProps", "mergeProps"]);

/**
 * Where a project component invokes the prop `name` it is handed: the child's
 * side of ADR 0442's walk through `props.<name>`. Each invocation is placed
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
      const cause = placementCause(project, enclosingFunction(parent), parent, depth + 1, seen, false).replace(/^via-callers:/, "");
      // A read: which package runs the function that calls the prop (Solid's own effect, or another's).
      if (readSite && cause === "package-callback") {
        const argument = usesOf(project, enclosingFunction(parent)).find((use) => use.kind === "argument");
        return `package-callback:${SOLID_PACKAGES.has(argument?.origin) ? "solid-js" : "other-package"}`;
      }
      return cause;
    }
    const use = useOf(project, node);
    if (readSite && use.kind === "returned") return `returned-from:${returnedFrom(project, enclosingFunction(node))}`;
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
 * How the census of the component enclosing `fn` escapes (ADR 0331, 0445):
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

/** The packages Solid's own primitives are declared in (`untrack` is declared by the signals package, re-exported by solid-js). */
const SOLID_PACKAGES = new Set(["package:solid-js", "package:@solidjs/signals", "package:@solidjs/web"]);

/** The function that returns a prop (`() => props.x`): `untrack`'s argument, a getter, another callback, a call's argument. */
function returnedFrom(project, thunk) {
  if (!thunk) return "unknown";
  const use = holderUse(project, thunk);
  const argument = usesOf(project, thunk).find((one) => one.kind === "argument");
  if (argument?.name === "untrack" && SOLID_PACKAGES.has(argument.origin)) return "untrack";
  return use || "unknown";
}

/**
 * Why the callers of the component holding a read cannot be enumerated: each
 * of these alone defeats the census (ADR 0331), so a component with more than
 * one is a different cause from a component with one (`spread-caller` is a
 * non-exported component rendered with a spread, `spread-caller+exported` an
 * exported one). The labels the other cases stand for are left as they were.
 */
function propsEscapeCause(project, fn, context = "", read = null) {
  const plain = context !== "condition";
  let component = fn;
  while (component && !isComponentLike(component)) component = enclosingFunction(component);
  if (!component) return plain ? notInComponentCause(project, fn) : "not-in-component";
  const census = componentCensus(project, component);
  const exported = census.exported ? "+exported" : "";
  const reference = census.other.length ? `non-jsx-reference:${census.other[0].kind}` : "";
  if (census.tags.some(jsxTagSpreads)) return `spread-caller${reference ? `+${reference}` : ""}${exported}`;
  if (reference) return reference + (plain && ["object-member", "PropertyAccessExpression"].includes(census.other[0].kind) ? exported : "");
  if (!census.tags.length) return census.exported ? "exported-unrendered" : "unrendered";
  if (census.exported) return read ? `exported-with-tag-callers:${callerValuesCause(project, census, read, component)}` : "exported-with-tag-callers";
  return read && plain ? `other:${callerValuesCause(project, census, read, component, true)}` : "other";
}

/**
 * What the tags that render a component pass for the prop a read names
 * (`props.projectId`): the props of an exported or a private component, a
 * callback's parameter, a local binding, or nothing. A component whose
 * callers are all known still cannot be certified when what they pass comes
 * from a caller whose own callers are not.
 */
function callerValuesCause(project, census, read, component, inCallbacks = false) {
  let node = read;
  while (ts.isElementAccessExpression(node) || ts.isCallExpression(node) || ts.isNonNullExpression(node) || (ts.isPropertyAccessExpression(node) && !ts.isIdentifier(node.expression))) node = node.expression;
  if (!ts.isPropertyAccessExpression(node)) return "prop-unnamed";
  // The read must be of the component's own props parameter; a callback's parameter or a local is another read.
  let declaration = project.symbol(node.expression)?.declarations?.[0];
  if (!declaration || !ts.isParameter(declaration)) return "read-of-local";
  if (enclosingFunction(declaration) !== component) return "read-of-callback-parameter";
  const values = callerValues(project, census, node.name.text, 0);
  if (values.has("exported-prop")) return "forwarded-from-exported";
  // Every tag written in a callback a package runs (`<Errored fallback={(err) => <Card .../>}>`): the package decides when the component renders.
  const inCallback = inCallbacks && census.tags.length > 0 && census.tags.every((use) => renderedInPackageCallback(project, use.node)) ? ":in-package-callback" : "";
  if (values.has("project-call-result")) return `callers-pass-project-call-result${inCallback}`;
  if (values.size === 1) return ({ omitted: "callers-omit-prop", "local-binding": "callers-pass-local-binding", "callback-parameter": `callers-pass-callback-parameter${inCallback}` }[[...values][0]] ?? "callers-pass-other");
  return "callers-pass-other";
}

/** Whether the JSX tag `node` is written in a function a package's component or function is handed (an `Errored` fallback). */
function renderedInPackageCallback(project, node) {
  const holder = enclosingFunction(node);
  if (!holder) return false;
  const use = useOf(project, holder);
  return (use.kind === "component-prop" || use.kind === "argument") && use.origin.startsWith("package");
}

/** An expression without its parentheses, assertions and non-null marks. */
function peelDown(expression) {
  while (ts.isParenthesizedExpression(expression) || ts.isAsExpression(expression) || ts.isNonNullExpression(expression) || ts.isSatisfiesExpression(expression)) expression = expression.expression;
  return expression;
}

/** The sources of what each tag that renders a component passes for `name`. */
function callerValues(project, census, name, depth) {
  const kinds = new Set();
  for (const use of census.tags) {
    const attribute = use.node.parent.attributes?.properties.find((property) => ts.isJsxAttribute(property) && property.name.getText() === name);
    if (!attribute) {
      kinds.add("omitted");
      continue;
    }
    const value = attribute.initializer && ts.isJsxExpression(attribute.initializer) ? attribute.initializer.expression : attribute.initializer;
    const expression = value && ts.isCallExpression(peelDown(value)) ? peelDown(value) : null;
    const callee = expression && project.symbol(ts.isPropertyAccessExpression(expression.expression) ? expression.expression.name : expression.expression)?.declarations?.[0];
    if (callee && project.originOf(ts.isPropertyAccessExpression(expression.expression) ? expression.expression.name : expression.expression) === "project" && bindingKind(callee) === "function") kinds.add("project-call-result");
    else kinds.add(value ? valueSource(project, value, depth) : "literal");
  }
  return kinds;
}

const SOURCE_ORDER = ["exported-prop", "private-prop", "callback-parameter", "local-binding", "module-binding", "omitted", "literal"];

/** The strongest source of the identifiers in an expression, by their declarations. */
function valueSource(project, expression, depth = 0, seen = new Set()) {
  const found = new Set();
  const visit = (node) => {
    if (ts.isIdentifier(node) && !(ts.isPropertyAccessExpression(node.parent) && node.parent.name === node) && !(ts.isPropertyAssignment(node.parent) && node.parent.name === node)) {
      let declaration = project.symbol(node)?.declarations?.[0];
      if (declaration && ts.isBindingElement(declaration)) {
        let pattern = declaration.parent;
        while (pattern && !ts.isParameter(pattern) && !ts.isVariableDeclaration(pattern)) pattern = pattern.parent;
        if (pattern && ts.isParameter(pattern)) declaration = pattern;
      }
      if (declaration && ts.isParameter(declaration)) {
        const owner = enclosingFunction(declaration);
        if (!owner || !isComponentLike(owner)) found.add("callback-parameter");
        else {
          // A private component's prop is what its own callers pass, up to an exported component's.
          const owned = componentCensus(project, owner);
          const member = ts.isPropertyAccessExpression(node.parent) && node.parent.expression === node ? node.parent.name.text : null;
          if (owned.exported) found.add("exported-prop");
          else if (member && depth < 3 && owned.tags.length) {
            const deeper = callerValues(project, owned, member, depth + 1);
            found.add(SOURCE_ORDER.find((kind) => deeper.has(kind)) ?? "private-prop");
          } else found.add("private-prop");
        }
      } else if (declaration && (ts.isImportSpecifier(declaration) || ts.isImportClause(declaration))) found.add("module-binding");
      else if (declaration) {
        // A local binding is what its initializer is made of (`const item = createMemo(() => props.items[0])`).
        const initializer = ts.isVariableDeclaration(declaration) ? declaration.initializer : null;
        if (initializer && depth < 3 && !seen.has(declaration)) {
          seen.add(declaration);
          found.add(valueSource(project, initializer, depth + 1, seen));
        } else found.add("local-binding");
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(expression);
  return SOURCE_ORDER.find((kind) => found.has(kind)) ?? "literal";
}

/**
 * The component a whole-component finding (SC1003, SC1004, SC5003) is about,
 * and why its callers are unknown: exported, spread into, referenced outside
 * JSX, rendered by no tag, or private and rendered by tags from a callback
 * or only from components. A function with no component's name (a callback
 * that returns JSX) is its own kind.
 */
function componentFlavorCause(project, fn) {
  if (!fn || !(isComponentLike(fn) || /^[A-Z]/.test(functionName(fn) ?? ""))) return "unnamed-function";
  const census = componentCensus(project, fn);
  const parts = [];
  if (census.tags.some(jsxTagSpreads)) parts.push("spread-caller");
  if (census.other.length) parts.push(`non-jsx-reference:${census.other[0].kind}`);
  if (census.exported) parts.push("exported");
  if (parts.length) return parts.join("+");
  if (!census.tags.length) return "unrendered";
  return census.tags.every((use) => {
    const caller = enclosingFunction(use.node.parent);
    return caller && isComponentLike(caller);
  })
    ? "private:callers-in-components"
    : "private:callers-in-callbacks";
}

/** Whether the project calls `render` or `hydrate` of `@solidjs/web` anywhere (SC5003's premise). */
function hasRenderRoot(project) {
  for (const source of project.sources) {
    if (!/\b(render|hydrate)\b/.test(source.text)) continue;
    let found = false;
    const visit = (node) => {
      if (found) return;
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && ["render", "hydrate"].includes(node.expression.text) && project.originOf(node.expression) === "package:@solidjs/web") {
        found = true;
        return;
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
    if (found) return true;
  }
  return false;
}

/**
 * A read in a function no census calls a component (it renders no JSX): the
 * nearest enclosing function with a component's name, and what keeps that
 * one's callers from being enumerated beyond its being rendered by tags.
 */
function notInComponentCause(project, fn) {
  let named = fn;
  while (named && !/^[A-Z]/.test(functionName(named) ?? "")) named = enclosingFunction(named);
  if (!named) return "not-in-component:anonymous";
  const census = componentCensus(project, named);
  const parts = [];
  if (census.tags.some(jsxTagSpreads)) parts.push("spread-caller");
  if (census.other.length) parts.push(`non-jsx-reference:${census.other[0].kind}`);
  if (census.exported) parts.push("exported");
  if (!census.tags.length) parts.push("unrendered");
  return ["not-in-component", ...parts].join("+");
}

const dynamicImports = new WeakMap();

/** Whether some project file loads `file` with `import("...")` (`lazy(() => import("./Page"))`), resolved as TypeScript resolves it. */
function importedDynamically(project, file) {
  let loaded = dynamicImports.get(project.program);
  if (!loaded) {
    loaded = new Set();
    dynamicImports.set(project.program, loaded);
    for (const source of project.sources) {
      if (!source.text.includes("import(")) continue;
      const visit = (node) => {
        if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword && ts.isStringLiteralLike(node.arguments[0])) {
          const resolved = ts.resolveModuleName(node.arguments[0].text, source.fileName, project.options, ts.sys).resolvedModule;
          if (resolved) loaded.add(resolved.resolvedFileName);
        }
        ts.forEachChild(node, visit);
      };
      visit(source);
    }
  }
  return loaded.has(file.fileName);
}

/**
 * An exported function that holds an unowned primitive (ADR 0331): a
 * component or another function, a default or a named export, and whether
 * a project file loads its module lazily.
 */
function exportedHolderCause(project, site) {
  let holder = enclosingFunction(site);
  while (holder && !usesOf(project, holder).some((use) => use.kind === "exported") && enclosingFunction(holder)) holder = enclosingFunction(holder);
  if (!holder) return "module-level";
  const statement = ts.isVariableDeclaration(holder.parent) ? holder.parent.parent?.parent : holder;
  const isDefault = hasModifier(holder, ts.SyntaxKind.DefaultKeyword) || ts.isExportAssignment(peel(holder).parent ?? holder);
  const lazy = importedDynamically(project, holder.getSourceFile());
  const named = statement && hasModifier(statement, ts.SyntaxKind.ExportKeyword);
  const reached = !lazy && reachedFromLazyRoute(project, holder);
  return [isComponentLike(holder) || /^[A-Z]/.test(functionName(holder) ?? "") ? "component" : "function", isDefault ? "default-export" : named ? "named-export" : "export-list", lazy ? "lazy-loaded" : reached ? "called-from-lazy-route" : "not-lazy"].join(":");
}

/** Whether a function is called, up to three calls up, from the default export of a module a project file loads with `import()`. */
function reachedFromLazyRoute(project, fn, depth = 0, seen = new Set()) {
  if (depth > 3 || seen.has(fn)) return false;
  seen.add(fn);
  for (const use of usesOf(project, fn)) {
    if (use.kind !== "called") continue;
    const caller = enclosingFunction(use.call);
    if (!caller) continue;
    const isDefault = hasModifier(caller, ts.SyntaxKind.DefaultKeyword) || ts.isExportAssignment(peel(caller).parent ?? caller);
    if ((isDefault && importedDynamically(project, caller.getSourceFile())) || reachedFromLazyRoute(project, caller, depth + 1, seen)) return true;
  }
  return false;
}

/**
 * An unowned primitive in a project function (ADR 0331): whether the call is
 * to a Solid primitive or a package's, and what the function holding it is:
 * an exported component or helper, a callback, a ref, a helper only called.
 */
function unownedPrimitiveCause(project, site) {
  const call = callAt(site, null);
  const callee = call ? project.callee(call) : null;
  const via = !callee ? "unresolved" : SOLID_PACKAGES.has(callee.origin) ? "solid-primitive" : callee.origin.startsWith("package") ? "package-primitive" : callee.origin;
  const holder = enclosingFunction(site);
  if (!holder) return `${via}:module-level`;
  const uses = usesOf(project, holder);
  if (uses.some((use) => use.kind === "exported")) return `${via}:${exportedHolderCause(project, site)}`;
  const ref = uses.find((use) => use.kind === "intrinsic-attribute" && use.name === "ref");
  const kind = ref ? "ref-callback" : holderUse(project, holder) || uses.map((use) => use.kind).find((one) => one !== "exported") || "unreferenced";
  return `${via}:${kind}`;
}

/** Where a call's function value is used: the origin class of the callee it is handed to, or how it is held. */
function holderUse(project, holder) {
  if (ts.isGetAccessorDeclaration(holder) || ts.isSetAccessorDeclaration(holder)) return "getter";
  const uses = usesOf(project, holder);
  const kinds = new Set(uses.map((use) => use.kind));
  const argument = uses.find((use) => use.kind === "argument");
  if (uses.some(isPromiseContinuation)) return "promise-continuation";
  if (argument) {
    if (argument.origin === "lib-dom") return "dom-callback";
    if (argument.origin.startsWith("lib")) return "sync-callback";
    return argument.origin === "project" ? "project-callback" : "package-callback";
  }
  if (kinds.has("object-member")) return "member";
  if (kinds.has("returned")) return "returned";
  if (kinds.has("component-prop")) return "component-prop";
  if (kinds.has("intrinsic-handler")) return "handler";
  return "";
}

/** Whether the read can be skipped: under a branch, a short circuit, a loop, or after an early return of its function. */
function onSomePaths(holder, read) {
  for (let node = read.parent; node && node !== holder; node = node.parent) {
    if (ts.isIfStatement(node) || ts.isConditionalExpression(node) || ts.isSwitchStatement(node) || ts.isTryStatement(node) || ts.isIterationStatement(node, false)) return true;
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind) && node.right.pos <= read.pos) return true;
  }
  let early = false;
  const visit = (node) => {
    if (early || node.pos >= read.pos) return;
    if (node !== holder && isFunction(node)) return;
    if (ts.isReturnStatement(node) || ts.isThrowStatement(node)) early = true;
    ts.forEachChild(node, visit);
  };
  visit(holder.body ?? holder);
  return early;
}

/** The node of the finding's evidence "<function> reads the reactive accessor": where the read a call propagates is written. */
function evidenceRead(project, finding) {
  const location = (finding.evidence ?? []).find((entry) => / reads the reactive accessor$/.test(entry.message))?.location;
  const file = location && project.file(location.path);
  if (!file) return null;
  const start = byteToChar(file, location.startByte);
  let node = file;
  const visit = (child) => {
    if (child.getStart(file) <= start && child.end > start) {
      node = child;
      ts.forEachChild(child, visit);
    }
  };
  ts.forEachChild(file, visit);
  return node === file ? null : node;
}

/**
 * Where the read that a call to a project function propagates sits in that
 * function (a direct read the call reaches): after an await, in a callback
 * or getter it registers or returns, on some paths only, or in its body.
 */
function readInCalleeCause(project, finding) {
  const read = evidenceRead(project, finding);
  if (!read) return "read-unlocated";
  const holder = enclosingFunction(read);
  if (!holder) return "module-level";
  if (afterSuspension(holder, read)) return "after-await";
  const use = holderUse(project, holder);
  if (use) return use;
  return onSomePaths(holder, read) ? "some-paths" : "body";
}

/**
 * Where an unplaced read is written in the component that holds it: in an
 * attribute (a handler's or ref's call argument, a component's prop, an
 * element's), a child expression, or a statement of the body.
 */
function readPositionCause(site) {
  let call = false;
  for (let node = site; node.parent && !isFunction(node.parent); node = node.parent) {
    const parent = node.parent;
    if (ts.isCallExpression(parent) && parent.arguments.includes(node)) call = true;
    if (ts.isJsxAttribute(parent)) {
      const tag = parent.parent.parent.tagName;
      const intrinsic = ts.isIdentifier(tag) && /^[a-z]/.test(tag.text);
      const name = parent.name.getText();
      const kind = /^on[A-Z]/.test(name) && intrinsic ? "handler" : name === "ref" ? "ref" : intrinsic ? "intrinsic" : "component";
      return `jsx-attribute:${kind}${call ? "-call-argument" : ""}`;
    }
    if (ts.isJsxExpression(parent) && (ts.isJsxElement(parent.parent) || ts.isJsxFragment(parent.parent))) return call ? "jsx-child-call-argument" : "jsx-child";
  }
  return "body";
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
  const base = leafCalleeBase(project, site);
  return LEAF_REFINED.has(base) ? leafRefinement(project, site) : base;
}

function leafCalleeBase(project, site) {
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

// ---------------------------------------------------------------------------
// Leaf callbacks: the call the leaf-scope walk cannot resolve (SC9012
// `leaf-callback-body`). The checker's walk (cleanup.rs `helper_forbidden_operations`)
// follows a project function's body and an exact receiver's method; it stops at
// a construction of a project or package class, a member whose declaration is a
// type member or lives in a package, a callee that is a parameter, a
// destructured value or a call's result, and a package function. The reading
// below applies that order to the call at the site and, for a callee whose
// body the walk enters, to the calls in that body. It models what the walk
// stops at, not every standard-library answer: a body with no stop found is
// `via:unmodeled`, never a mechanism.

/** The labels `leafRefinement` reads again; every other leaf label belongs to another case. */
const LEAF_REFINED = new Set([
  "package-member",
  "project-member",
  "project-function",
  "project-destructured-binding",
  "project-destructured-call-result",
  "project-parameter",
  "project-parameter-member",
  "package-function",
  "project-call-result",
  "project-binding",
  "project-destructured",
  "unresolved",
]);

const unwrap = (node) => {
  while (node && (ts.isParenthesizedExpression(node) || ts.isNonNullExpression(node) || ts.isAsExpression(node) || ts.isSatisfiesExpression(node) || ts.isTypeAssertionExpression(node))) node = node.expression;
  return node;
};
const declarationOf = (project, node) => project.symbol(node)?.declarations?.[0];
/** `project`, `package`, `lib` (a TypeScript lib file) or `unresolved`. */
const originClass = (where) => (where === "unresolved" ? where : where.startsWith("package") ? "package" : where.startsWith("lib") ? "lib" : where);
const isSolidPackage = (where) => where === "package:solid-js" || where === "package:@solidjs/signals";
const isConstDeclaration = (declaration) => ts.isVariableDeclaration(declaration) && ts.isVariableDeclarationList(declaration.parent) && (declaration.parent.flags & ts.NodeFlags.Const) !== 0;

/** The function a declaration's value is, when the declaration holds its body. */
function bodyOfDeclaration(project, declaration, depth = 0) {
  if (!declaration || depth > 3) return null;
  if (ts.isFunctionDeclaration(declaration) || ts.isMethodDeclaration(declaration)) return declaration.body ? declaration : null;
  const initializer = (ts.isVariableDeclaration(declaration) || ts.isPropertyAssignment(declaration)) && declaration.initializer && unwrap(declaration.initializer);
  if (initializer) return isFunction(initializer) ? initializer : null;
  if (ts.isShorthandPropertyAssignment(declaration)) {
    const value = project.checker.getShorthandAssignmentValueSymbol(declaration)?.declarations?.[0];
    return value ? bodyOfDeclaration(project, value, depth + 1) : null;
  }
  return null;
}

/** The calls and constructions `fn` runs in its own body (a nested function runs later). */
function directCalls(fn) {
  const found = [];
  const visit = (node) => {
    if (isFunction(node) || ts.isClassLike(node)) return;
    if (ts.isCallExpression(node) || ts.isNewExpression(node)) found.push(node);
    ts.forEachChild(node, visit);
  };
  if (fn.body) ts.isBlock(fn.body) ? ts.forEachChild(fn.body, visit) : visit(fn.body);
  return found.sort((a, b) => a.getStart() - b.getStart());
}

/** Where an async function's first await that every run reaches ends; later calls run from a continuation. */
function firstUnconditionalAwaitEnd(fn) {
  if (!hasModifier(fn, ts.SyntaxKind.AsyncKeyword) || !fn.body) return Infinity;
  let end = Infinity;
  const visit = (node) => {
    if (end !== Infinity || isFunction(node) || ts.isClassLike(node)) return;
    if (ts.isAwaitExpression(node)) {
      end = node.getEnd();
      return;
    }
    if (ts.isIfStatement(node)) return visit(node.expression);
    if (ts.isIterationStatement(node, false) || ts.isSwitchStatement(node)) return;
    if (ts.isConditionalExpression(node)) return visit(node.condition);
    if (ts.isBinaryExpression(node) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(node.operatorToken.kind)) return visit(node.left);
    if (ts.isTryStatement(node)) return;
    ts.forEachChild(node, visit);
  };
  ts.isBlock(fn.body) ? ts.forEachChild(fn.body, visit) : visit(fn.body);
  return end;
}

/** The root of a member chain (`a.b[c].d` is `a`) and whether the chain has links. */
function chainRoot(expression) {
  let root = unwrap(expression);
  let links = 0;
  while (root && (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root))) {
    root = unwrap(root.expression);
    links++;
  }
  return { root, links };
}

/** What a member call's receiver is: a package value, an instance a project binding holds, a parameter, a call's result. */
function receiverKind(project, access) {
  const { root, links } = chainRoot(access.expression);
  const prefix = links ? "chained-" : "";
  if (!root) return `${prefix}other`;
  if (ts.isCallExpression(root) || ts.isNewExpression(root)) return `${prefix}call-result`;
  if (root.kind === ts.SyntaxKind.ThisKeyword) return `${prefix}this`;
  if (!ts.isIdentifier(root)) return `${prefix}other`;
  const declaration = declarationOf(project, root);
  if (!declaration) return `${prefix}unresolved`;
  const where = origin(declaration);
  if (originClass(where) === "package" || originClass(where) === "lib") return `${prefix}package-${ts.isSourceFile(declaration) || ts.isNamespaceImport(declaration) || ts.isModuleDeclaration(declaration) ? "namespace" : ts.isClassDeclaration(declaration) ? "class" : "value"}`;
  if (ts.isParameter(declaration)) return `${prefix}parameter`;
  if (ts.isBindingElement(declaration)) return `${prefix}destructured`;
  if (ts.isVariableDeclaration(declaration)) {
    if (!isConstDeclaration(declaration)) return `${prefix}let-binding`;
    const init = declaration.initializer && unwrap(declaration.initializer.kind === ts.SyntaxKind.AwaitExpression ? declaration.initializer.expression : declaration.initializer);
    if (init && ts.isNewExpression(init)) return `${prefix}instance-new`;
    if (init && ts.isCallExpression(init)) return `${prefix}instance-call`;
  }
  return `${prefix}other`;
}

/**
 * Whether the package a declaration file belongs to ships runtime code (its
 * manifest names a JavaScript entry), or only types (`@types/three`, whose
 * `exports` name files it does not ship, and `@webgpu/types`).
 */
const shipsCode = new Map();
function packageShipsCode(declaration) {
  const file = declaration.getSourceFile().fileName;
  const marker = file.lastIndexOf("/node_modules/");
  if (marker < 0) return true;
  const rest = file.slice(marker + "/node_modules/".length).split("/");
  const name = rest[0].startsWith("@") ? `${rest[0]}/${rest[1]}` : rest[0];
  const manifest = `${file.slice(0, marker)}/node_modules/${name}/package.json`;
  if (!shipsCode.has(manifest)) {
    let found = false;
    try {
      const json = JSON.parse(readFileSync(manifest, "utf8"));
      const entries = [json.main, json.module, json.browser, ...JSON.stringify(json.exports ?? "").match(/"[^"]*"/g)?.map((text) => text.slice(1, -1)) ?? []];
      found = !name.startsWith("@types/") && entries.some((entry) => typeof entry === "string" && /\.(?:c|m)?js$/.test(entry));
    } catch {
      found = false;
    }
    shipsCode.set(manifest, found);
  }
  return shipsCode.get(manifest);
}

/** A package member's receiver: the package's own value (a namespace, a class, an exported object), or a value a project binding holds and how it got it. */
const packageReceiver = (kind, project, access) => {
  const root = kind.replace(/^chained-/, "");
  if (root.startsWith("package-")) return `static-${root.slice(8)}`;
  let named = { "instance-call": "instance-call", "call-result": "call-expression", "instance-new": "instance-new", destructured: "instance-destructured" }[root] ?? root;
  if (named === "instance-call" && receiverCallOrigin(project, access) === "project") named += "-project";
  return kind.startsWith("chained-") ? `chained-${named}` : named;
};

/** Whose call a receiver binding holds the result of: a project function's, or another's (a package function's, a member's). */
function receiverCallOrigin(project, access) {
  const { root } = chainRoot(access.expression);
  const declaration = root && ts.isIdentifier(root) ? declarationOf(project, root) : null;
  const init = declaration && ts.isVariableDeclaration(declaration) && declaration.initializer && unwrap(declaration.initializer.kind === ts.SyntaxKind.AwaitExpression ? declaration.initializer.expression : declaration.initializer);
  if (!init || !ts.isCallExpression(init)) return "unknown";
  const callee = unwrap(init.expression);
  if (ts.isPropertyAccessExpression(callee)) return "member";
  return originClass(project.originOf(callee));
}

/** Whether a body-holding member is one the walk enters: an exact instance's own method, or a member of an object literal a binding holds. */
function enteredMember(project, access, declaration) {
  const body = bodyOfDeclaration(project, declaration);
  if (!body) return null;
  const receiver = unwrap(access.expression);
  if (receiver.kind === ts.SyntaxKind.ThisKeyword) return ts.isMethodDeclaration(declaration) && ts.isClassLike(declaration.parent) ? body : null;
  if (!ts.isIdentifier(receiver)) return null;
  const holder = declarationOf(project, receiver);
  if (!holder || !isConstDeclaration(holder) || !holder.initializer) return null;
  const init = unwrap(holder.initializer);
  if (ts.isNewExpression(init)) {
    const owner = declarationOf(project, unwrap(init.expression));
    return owner && ts.isClassDeclaration(owner) && declaration.parent === owner ? body : null;
  }
  return ts.isObjectLiteralExpression(declaration.parent) ? body : null;
}

/** Where the value an object member holds comes from: a Solid primitive's result, another call's result, a parameter. */
function memberValueSource(project, declaration) {
  let value = declaration;
  if (ts.isShorthandPropertyAssignment(declaration)) value = project.checker.getShorthandAssignmentValueSymbol(declaration)?.declarations?.[0];
  else if (ts.isPropertyAssignment(declaration)) {
    const init = unwrap(declaration.initializer);
    value = ts.isIdentifier(init) ? declarationOf(project, init) : init;
  }
  if (!value) return "unresolved";
  if (ts.isParameter(value)) return "parameter";
  let init = null;
  if (ts.isBindingElement(value)) {
    let pattern = value.parent;
    while (pattern && !ts.isParameter(pattern) && !ts.isVariableDeclaration(pattern)) pattern = pattern.parent;
    if (pattern && ts.isParameter(pattern)) return "parameter";
    init = pattern?.initializer;
  } else if (ts.isVariableDeclaration(value)) init = value.initializer;
  else if (ts.isCallExpression(value)) init = value;
  init = init && unwrap(init);
  if (init && ts.isCallExpression(init)) {
    const callee = unwrap(init.expression);
    const where = project.originOf(ts.isPropertyAccessExpression(callee) ? callee.name : callee);
    return isSolidPackage(where) ? "solid-primitive-result" : `call-result-${originClass(where)}`;
  }
  return "other";
}

/**
 * A standard-library member call in a body of another file than the leaf
 * callback's, whose receiver the checker's per-file call facts do not carry
 * (probed on the checker: `(a ?? b).trim()`, `String(1).trim()`, a template or a
 * literal receiver, and a member of a binding an `await` initializes, stop the walk in an
 * imported helper and not in one written beside the leaf callback).
 */
function unreadStdMember(project, call, sameFile) {
  if (sameFile) return null;
  const callee = unwrap(call.expression);
  if (!ts.isPropertyAccessExpression(callee)) return null;
  const receiver = unwrap(callee.expression);
  if (ts.isCallExpression(receiver)) return ts.isIdentifier(unwrap(receiver.expression)) ? "call-result" : null;
  if (ts.isTemplateExpression(receiver) || ts.isNoSubstitutionTemplateLiteral(receiver) || ts.isStringLiteral(receiver)) return "literal";
  if (ts.isBinaryExpression(receiver)) return "binary-expression";
  if (ts.isIdentifier(receiver)) {
    const holder = declarationOf(project, receiver);
    return holder && ts.isVariableDeclaration(holder) && holder.initializer && ts.isAwaitExpression(holder.initializer) ? "awaited-binding" : null;
  }
  return null;
}

/** The library members whose run re-enters code the walk cannot see (cleanup.rs `synchronously_dispatching_dom_member`). */
const LIB_DISPATCH = new Set(["focus", "blur", "click", "dispatchEvent", "checkValidity", "reportValidity", "showModal", "showPopover", "hidePopover", "togglePopover"]);

/**
 * What the walk does at one call: `{ blocked }` it stops, `{ follow }` it enters
 * a function's body, `{}` it resolves the call.
 */
function walkAt(project, call) {
  if (ts.isNewExpression(call)) return originClass(project.originOf(unwrap(call.expression))) === "lib" ? {} : { blocked: `new-${originClass(project.originOf(unwrap(call.expression)))}-class` };
  const callee = unwrap(call.expression);
  if (isFunction(callee)) return { follow: callee };
  if (ts.isElementAccessExpression(callee)) return { blocked: "element-access-callee" };
  if (ts.isPropertyAccessExpression(callee)) {
    const declaration = declarationOf(project, callee.name);
    if (!declaration) return { blocked: "member-unresolved" };
    const where = origin(declaration);
    if (originClass(where) === "lib") {
      if (callee.name.text === "then" && /PromiseLike/.test(declaration.parent?.name?.text ?? "")) return { blocked: "promise-like-then" };
      return LIB_DISPATCH.has(callee.name.text) && where === "lib-dom" ? { blocked: "dom-dispatch" } : {};
    }
    if (originClass(where) === "package") return isSolidPackage(where) ? {} : { blocked: `member-package/${packageReceiver(receiverKind(project, callee), project, callee)}${packageShipsCode(declaration) ? "" : "-types-only"}` };
    const entered = enteredMember(project, callee, declaration);
    if (entered) return { follow: entered };
    if (bodyOfDeclaration(project, declaration)) return { blocked: `member-body-not-entered/${receiverKind(project, callee)}` };
    if (ts.isGetAccessorDeclaration(declaration)) return { blocked: "member-getter" };
    if (ts.isPropertySignature(declaration) || ts.isMethodSignature(declaration)) return { blocked: "member-signature" };
    if (ts.isMethodDeclaration(declaration) || ts.isFunctionDeclaration(declaration)) return { blocked: "member-declaration-only" };
    return { blocked: `member-value/${memberValueSource(project, declaration)}` };
  }
  if (!ts.isIdentifier(callee)) return { blocked: "other-callee" };
  const declaration = declarationOf(project, callee);
  if (!declaration) return { blocked: "unresolved" };
  const where = origin(declaration);
  if (originClass(where) === "lib") return {};
  if (originClass(where) === "package") return isSolidPackage(where) ? {} : { blocked: `function-package${packageShipsCode(declaration) ? "" : "-types-only"}` };
  const body = bodyOfDeclaration(project, declaration);
  if (body) return { follow: body };
  if (ts.isParameter(declaration)) return { blocked: "parameter" };
  if (ts.isBindingElement(declaration)) return destructuredWalk(project, declaration);
  if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
    const init = unwrap(declaration.initializer);
    if (ts.isCallExpression(init)) {
      const made = originClass(project.originOf(ts.isPropertyAccessExpression(unwrap(init.expression)) ? unwrap(init.expression).name : unwrap(init.expression)));
      return made === "package" && isSolidPackage(project.originOf(unwrap(init.expression))) ? {} : { blocked: "call-result" };
    }
  }
  return { blocked: "binding" };
}

/** A callee bound by destructuring: from a parameter's own pattern, a parameter's value, another binding, or a call's result. */
function destructuredWalk(project, element) {
  let pattern = element.parent;
  let array = false;
  while (pattern && !ts.isParameter(pattern) && !ts.isVariableDeclaration(pattern)) {
    if (ts.isArrayBindingPattern(pattern)) array = true;
    pattern = pattern.parent;
  }
  const shape = array ? "array" : "object";
  if (!pattern) return { blocked: `destructured-${shape}` };
  if (ts.isParameter(pattern)) return { blocked: `destructured-${shape}-in-parameter` };
  const init = pattern.initializer && unwrap(pattern.initializer.kind === ts.SyntaxKind.AwaitExpression ? pattern.initializer.expression : pattern.initializer);
  if (!init) return { blocked: `destructured-${shape}` };
  if (ts.isIdentifier(init)) {
    const holder = declarationOf(project, init);
    return { blocked: `destructured-${shape}-from-${holder && ts.isParameter(holder) ? "parameter-value" : "binding"}` };
  }
  if (ts.isCallExpression(init)) {
    const callee = unwrap(init.expression);
    const where = project.originOf(ts.isPropertyAccessExpression(callee) ? callee.name : callee);
    if (isSolidPackage(where)) return {};
    // A project hook that is itself a binding (`const [Provider, useCamera] = createContextProvider(..)`) is read apart from a function.
    const hook = ts.isIdentifier(callee) ? declarationOf(project, callee) : null;
    const bound = originClass(where) === "project" && hook && !ts.isFunctionDeclaration(hook) && !(ts.isVariableDeclaration(hook) && hook.initializer && isFunction(unwrap(hook.initializer)));
    return { blocked: `destructured-${shape}-from-${originClass(where)}-${bound ? "bound-" : ""}call` };
  }
  return { blocked: `destructured-${shape}` };
}

/** The call, construction or function literal a leaf finding's location names, or how it sits in a leaf call. */
function leafTarget(site) {
  if (ts.isCallExpression(site) || ts.isNewExpression(site)) return { call: site };
  if (isFunction(site)) {
    let holder = site.parent;
    while (holder && ts.isParenthesizedExpression(holder)) holder = holder.parent;
    if (holder && ts.isCallExpression(holder) && unwrap(holder.expression) === site) return { literal: site };
  }
  let child = site;
  let parent = site.parent;
  while (parent) {
    if (ts.isCallExpression(parent) || ts.isNewExpression(parent)) return parent.expression === child ? { call: parent } : { argument: child, call: parent };
    const link =
      (ts.isPropertyAccessExpression(parent) && (parent.expression === child || parent.name === child)) ||
      ts.isElementAccessExpression(parent) ||
      ts.isParenthesizedExpression(parent) ||
      ts.isNonNullExpression(parent) ||
      ts.isAsExpression(parent) ||
      ts.isSatisfiesExpression(parent) ||
      ts.isTypeAssertionExpression(parent);
    if (!link) break;
    child = parent;
    parent = parent.parent;
  }
  return null;
}

/** The structure of a leaf finding: the stop at the call itself, or `via:` the stops in the body the walk enters. */
function leafRefinement(project, site) {
  const target = leafTarget(site);
  if (!target) return "no-target";
  let step;
  if (target.literal) step = { follow: target.literal };
  else if (target.argument) {
    const argument = unwrap(target.argument);
    const declaration = ts.isIdentifier(argument) ? declarationOf(project, argument) : null;
    const body = declaration && bodyOfDeclaration(project, declaration);
    step = body ? { follow: body } : { blocked: "callback-unresolved" };
  } else step = walkAt(project, target.call);
  if (step.blocked) return step.blocked;
  if (!step.follow) return "unexplained";
  const stops = new Set();
  const seen = new Set();
  const enter = (fn, depth) => {
    if (depth <= 0) return stops.add("depth");
    if (seen.has(fn)) return;
    seen.add(fn);
    const cutoff = firstUnconditionalAwaitEnd(fn);
    for (const call of directCalls(fn)) {
      if (call.getStart() >= cutoff) continue;
      const next = walkAt(project, call);
      if (next.blocked) stops.add(next.blocked);
      else if (next.follow) enter(next.follow, depth - 1);
      else {
        const unread = unreadStdMember(project, call, fn.getSourceFile() === site.getSourceFile());
        if (unread) stops.add(`std-member-unread-${unread}`);
      }
    }
  };
  enter(step.follow, 8);
  return `via:${stops.size ? [...stops].sort().join("+") : "unmodeled"}`;
}

/** What a computed call target indexes: an array held by a binding the function builds, a destructured one, a parameter, or an object's keyed member (`target[PROXY]()`). */
function computedTargetCause(project, site, fn) {
  let access = site;
  while (access && !ts.isElementAccessExpression(access)) access = access.parent;
  if (!access) return "no-element-access";
  const object = unwrap(access.expression);
  const type = project.checker.getTypeAtLocation(object);
  if (!project.checker.isArrayType(type) && !project.checker.isTupleType(type)) return "keyed-member";
  const declaration = ts.isIdentifier(object) ? declarationOf(project, object) : null;
  if (!declaration) return "unresolved-array";
  if (ts.isParameter(declaration)) return "parameter";
  if (ts.isBindingElement(declaration)) return "destructured";
  return ts.isVariableDeclaration(declaration) ? `local-binding/${placementCause(project, fn, site)}` : ts.SyntaxKind[declaration.kind];
}

/** How an exported function that dispatches on a parameter is entered: an override of a base class's method, a static method, another member. */
function exportedDispatchCause(project, fn) {
  if (!fn || !ts.isMethodDeclaration(fn) || !ts.isClassLike(fn.parent)) return "not-a-method";
  if (hasModifier(fn, ts.SyntaxKind.StaticKeyword)) return "static-method";
  const name = fn.name && ts.isIdentifier(fn.name) ? fn.name.text : null;
  const type = project.checker.getTypeAtLocation(fn.parent);
  const base = type && name ? project.checker.getBaseTypes(type).find((one) => one.getProperty(name)) : null;
  return base ? "override-of-base-method" : "instance-method";
}

/** The provenance of the rich argument a client hands a server function: a parameter's property, a call's result's property. */
function serverArgumentCause(project, site) {
  let root = unwrap(site);
  while (root && (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root))) root = unwrap(root.expression);
  if (root && ts.isCallExpression(root)) return "call-result-property";
  const declaration = root && ts.isIdentifier(root) ? declarationOf(project, root) : null;
  if (declaration && (ts.isParameter(declaration) || ts.isBindingElement(declaration))) return "parameter-property";
  return "other";
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

/** Whether a call is to Solid's `omit`/`splitProps`, a view of its first argument. */
function isSolidView(project, call) {
  const callee = ts.isPropertyAccessExpression(call.expression) ? call.expression.name : call.expression;
  if (!ts.isIdentifier(callee)) return false;
  const declaration = project.symbol(callee)?.declarations?.[0];
  return !!declaration && isSolidPackage(origin(declaration)) && ["omit", "splitProps"].includes(callee.text);
}

/**
 * What the expression spread onto an element (`{...operand}`) is, read from
 * its declaration. A Solid view of a parameter (`omit(props, ...)`,
 * `splitProps`'s rest) is `inline-omit` or `named-rest`; any other operand
 * takes its own kind: the result of a package function (`package-call-result`),
 * of a project function or callable (`project-call-result:<declaration>`), of a
 * call whose callee does not resolve, or a member of one of these.
 */
function spreadOperand(project, site, depth = 0) {
  const node = unwrap(site);
  if (depth > 3) return "unresolved-operand";
  if (ts.isCallExpression(node)) {
    if (isSolidView(project, node)) {
      const first = node.arguments[0] ? unwrap(node.arguments[0]) : null;
      const declaration = first && ts.isIdentifier(first) ? project.symbol(first)?.declarations?.[0] : null;
      if (!declaration || !ts.isParameter(declaration)) return "view-of-local";
      return depth === 0 ? "inline-omit" : "named-rest";
    }
    const callee = unwrap(node.expression);
    const target = ts.isPropertyAccessExpression(callee) ? callee.name : callee;
    const declaration = project.symbol(target)?.declarations?.[0];
    if (!declaration) return "unresolved-call-result";
    const where = origin(declaration);
    if (isSolidPackage(where)) return "solid-call-result";
    if (where.startsWith("package")) return "package-call-result";
    if (where !== "project") return `${where}-call-result`;
    const kinds = new Map([
      [ts.SyntaxKind.FunctionDeclaration, "function"],
      [ts.SyntaxKind.VariableDeclaration, "variable"],
      [ts.SyntaxKind.BindingElement, "destructured"],
      [ts.SyntaxKind.Parameter, "parameter"],
      [ts.SyntaxKind.PropertyAssignment, "member"],
      [ts.SyntaxKind.PropertySignature, "member"],
      [ts.SyntaxKind.MethodDeclaration, "member"],
    ]);
    return `project-call-result:${kinds.get(declaration.kind) ?? ts.SyntaxKind[declaration.kind]}`;
  }
  if (ts.isIdentifier(node)) {
    const declaration = project.symbol(node)?.declarations?.[0];
    if (!declaration) return "unresolved-operand";
    if (ts.isVariableDeclaration(declaration) && declaration.initializer) return spreadOperand(project, declaration.initializer, depth + 1);
    if (ts.isBindingElement(declaration)) {
      let pattern = declaration.parent;
      while (pattern && !ts.isVariableDeclaration(pattern) && !ts.isParameter(pattern)) pattern = pattern.parent;
      if (pattern && ts.isVariableDeclaration(pattern) && pattern.initializer) return spreadOperand(project, pattern.initializer, depth + 1);
      return "destructured-parameter";
    }
    if (ts.isParameter(declaration)) return "parameter";
    return "other-binding";
  }
  if (ts.isPropertyAccessExpression(node)) {
    let root = unwrap(node.expression);
    while (ts.isPropertyAccessExpression(root)) root = unwrap(root.expression);
    return `member-of:${ts.isIdentifier(root) ? spreadOperand(project, root, depth + 1) : "expression"}`;
  }
  if (ts.isObjectLiteralExpression(node)) return "object-literal";
  return "other-operand";
}

// ---------------------------------------------------------------------------
// SC9012: what a callee does with the argument a result is about (ADR 0320).

/** The call or `new` and the position of the argument at `site`. */
function argumentSlot(site) {
  const { node, parent } = peel(site);
  if (parent && (ts.isCallExpression(parent) || ts.isNewExpression(parent)) && parent.arguments?.includes(node)) {
    return { call: parent, index: parent.arguments.indexOf(node) };
  }
  return null;
}

/**
 * What TypeScript's resolved signature of `call` names: where it is declared
 * (`project`, `package`, `lib`, `unresolved`) and the function body to read,
 * when one is.
 */
function resolveCallee(project, call) {
  let declaration = project.checker.getResolvedSignature(call)?.declaration;
  if (!declaration) {
    const target = ts.isPropertyAccessExpression(call.expression) ? call.expression.name : call.expression;
    declaration = project.symbol(target)?.declarations?.[0];
  }
  if (!declaration) return { where: "unresolved", fn: null };
  const where = origin(declaration);
  let fn = isFunction(declaration) && declaration.body ? declaration : null;
  if (!fn) fn = declaration.symbol?.declarations?.find((candidate) => isFunction(candidate) && candidate.body) ?? null;
  return { where: where === "project" ? where : where.startsWith("package") ? "package" : "lib", fn, declaration, package: where.startsWith("package:") ? where.slice(8) : null };
}

/** Whether `expression` is a member of an imported namespace (`Tree.move`): an exact function, not a member of an object. */
function isNamespaceMember(project, expression) {
  if (!ts.isPropertyAccessExpression(expression)) return false;
  const receiver = project.symbol(expression.expression);
  return Boolean(receiver && receiver.flags & (ts.SymbolFlags.ValueModule | ts.SymbolFlags.NamespaceModule));
}

/** How a call that takes the argument is written: a member of an object, a function, a constructor; and where its target is declared. */
function calleeShape(project, call, owner) {
  const callee = resolveCallee(project, call);
  const where = callee.package ? `package:${callee.package}` : callee.where;
  if (ts.isNewExpression(call)) return `new-${where}`;
  const member = ts.isPropertyAccessExpression(call.expression) && !isNamespaceMember(project, call.expression);
  const closure = !member && enclosingFunction(call) !== owner ? "-in-closure" : "";
  return `${member ? "member" : calleeBinding(project, call)}-${where}${closure}`;
}

/** What the called name is bound to: a function declared as such, or a parameter or other binding holding a function value. */
function calleeBinding(project, call) {
  const expression = unparenthesized(call.expression);
  if (!ts.isIdentifier(expression) && !ts.isPropertyAccessExpression(expression)) return "function";
  const declaration = project.symbol(ts.isPropertyAccessExpression(expression) ? expression.name : expression)?.declarations?.[0];
  if (!declaration) return "function";
  if (ts.isParameter(declaration)) return "parameter";
  if (ts.isBindingElement(declaration)) return "binding";
  if (ts.isVariableDeclaration(declaration) && !(declaration.initializer && isFunction(unparenthesized(declaration.initializer)))) return "binding";
  return "function";
}

const USE_RANK = ["argument:member", "wrapped:argument:member", "argument:function", "argument:new", "wrapped", "computed-member", "member-chain", "alias", "destructured"];
const useRank = (use) => {
  const index = USE_RANK.findIndex((prefix) => use.startsWith(prefix));
  return index < 0 ? USE_RANK.length : index;
};

/**
 * The use of parameter `index` of `fn` that no member invocation describes
 * (parameter_use.rs, ADR 0320), by what the code does: handed to a call, with
 * the call's shape (`argument:member-project`: a member of another object), a
 * wrapper that is handed on, a computed member, an alias, a destructured
 * parameter. A whole argument of an exact project function is followed into
 * it. Null when the body has no such use.
 */
function parameterUse(project, fn, index, depth = 0, seen = new Set()) {
  const parameter = fn.parameters[index];
  if (!parameter) return fn.parameters.some((one) => one.dotDotDotToken) ? "rest-parameter" : "no-parameter";
  if (parameter.dotDotDotToken) return "rest-parameter";
  if (!ts.isIdentifier(parameter.name)) {
    const slots = parameter.name.elements ?? [];
    const flag = slots.some((slot) => slot.dotDotDotToken) ? "rest" : slots.some((slot) => slot.initializer) ? "default" : slots.some((slot) => !ts.isIdentifier(slot.name)) ? "nested" : "plain";
    return `destructured-parameter:${flag}`;
  }
  if (seen.has(fn)) return null;
  seen.add(fn);
  const found = fn.body ? referencesIn(project, fn.body, parameter.name) : [];
  const uses = found.map((reference) => referenceUse(project, fn, reference, depth, seen)).filter(Boolean);
  uses.sort((a, b) => useRank(a) - useRank(b) || (a < b ? -1 : 1));
  return uses[0] ?? null;
}

/** The unmodeled use one reference of a parameter makes, or null when it is a modeled or an unseen one. */
function referenceUse(project, owner, reference, depth, seen) {
  const { node, parent } = peel(reference);
  if (!parent) return null;
  if (ts.isPropertyAccessExpression(parent) && parent.expression === node) {
    const outer = peel(parent);
    const chained = (ts.isPropertyAccessExpression(outer.parent) || ts.isElementAccessExpression(outer.parent)) && outer.parent.expression === outer.node;
    return chained && enclosingFunction(reference) !== owner ? "member-chain-in-closure" : null;
  }
  if (ts.isElementAccessExpression(parent) && parent.expression === node) {
    return ts.isStringLiteralLike(parent.argumentExpression) || ts.isNumericLiteral(parent.argumentExpression) ? null : "computed-member";
  }
  const slot = argumentSlot(node);
  if (slot) {
    if (slot.call.arguments.slice(0, slot.index + 1).some(ts.isSpreadElement)) return `argument:${calleeShape(project, slot.call, owner)}-spread`;
    const callee = resolveCallee(project, slot.call);
    const inner = callee.fn;
    const exact = callee.where === "project" && inner && !ts.isNewExpression(slot.call) && !(ts.isPropertyAccessExpression(slot.call.expression) && !isNamespaceMember(project, slot.call.expression));
    if (exact && enclosingFunction(slot.call) === owner && depth < 6) return parameterUse(project, inner, slot.index, depth + 1, seen);
    return `argument:${calleeShape(project, slot.call, owner)}`;
  }
  if (ts.isSpreadElement(parent) && argumentSlot(parent)) return `spread-arguments:${calleeShape(project, argumentSlot(parent).call, owner)}`;
  if (ts.isSpreadElement(parent) || ts.isSpreadAssignment(parent) || ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent) || ts.isArrayLiteralExpression(parent)) {
    const kind = ts.isArrayLiteralExpression(parent) ? "array" : ts.isSpreadAssignment(parent) || ts.isSpreadElement(parent) ? "spread" : "object";
    let current = ts.isPropertyAssignment(parent) || ts.isShorthandPropertyAssignment(parent) || ts.isSpreadAssignment(parent) ? parent.parent : parent;
    for (; current && !isFunction(current); current = current.parent) {
      const slotOf = argumentSlot(current);
      if (slotOf) return `wrapped:${kind}:argument:${calleeShape(project, slotOf.call, owner)}`;
      if (ts.isStatement(current)) break;
    }
    return null;
  }
  if (ts.isVariableDeclaration(parent) && parent.initializer === node) {
    if (!ts.isIdentifier(parent.name)) return "destructured-alias";
    if (depth >= 6) return "alias";
    const aliased = referencesIn(project, owner.body, parent.name);
    const uses = aliased.map((one) => referenceUse(project, owner, one, depth + 1, seen)).filter(Boolean);
    uses.sort((a, b) => useRank(a) - useRank(b) || (a < b ? -1 : 1));
    return uses[0] ?? null;
  }
  if (ts.isConditionalExpression(parent) && parent.condition !== node) return referenceUse(project, owner, parent, depth + 1, seen);
  if (ts.isBinaryExpression(parent) && [ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken, ts.SyntaxKind.AmpersandAmpersandToken].includes(parent.operatorToken.kind)) {
    if (parent.right === node || parent.operatorToken.kind !== ts.SyntaxKind.AmpersandAmpersandToken) return referenceUse(project, owner, parent, depth + 1, seen);
  }
  return null;
}

/** Every identifier under `root` that resolves to the declaration `name` (a shorthand property reads its value's symbol). */
function referencesIn(project, root, name) {
  const symbol = project.checker.getSymbolAtLocation(name);
  const found = [];
  if (!symbol) return found;
  const visit = (node) => {
    if (ts.isIdentifier(node) && node.text === name.text && node !== name) {
      const target = ts.isShorthandPropertyAssignment(node.parent) && node.parent.name === node ? project.checker.getShorthandAssignmentValueSymbol(node.parent) : project.checker.getSymbolAtLocation(node);
      if (target === symbol) found.push(node);
    }
    ts.forEachChild(node, visit);
  };
  visit(root);
  return found;
}

/** The component enclosing `fn` (or `fn` itself), or null. */
function enclosingComponent(fn) {
  let component = fn;
  while (component && !isComponentLike(component)) component = enclosingFunction(component);
  return component ?? null;
}

/**
 * Whether a value read in `node` comes from a parameter of `component` (its
 * props), directly or through the local bindings and closures that hold it. A
 * component whose callers cannot be enumerated (ADR 0331, 0445) leaves such a
 * value unproven; one whose arguments do not come from its props is held by
 * what those arguments are.
 */
function readsParameterOf(project, node, component, depth = 0, seen = new Set()) {
  let found = false;
  const visit = (child) => {
    if (found) return;
    if (ts.isIdentifier(child)) {
      const symbol = ts.isShorthandPropertyAssignment(child.parent) && child.parent.name === child ? project.checker.getShorthandAssignmentValueSymbol(child.parent) : project.checker.getSymbolAtLocation(child);
      let declaration = symbol?.valueDeclaration ?? symbol?.declarations?.[0];
      while (declaration && ts.isBindingElement(declaration)) declaration = declaration.parent?.parent;
      if (declaration && !seen.has(declaration)) {
        seen.add(declaration);
        if (ts.isParameter(declaration) && declaration.parent === component) found = true;
        else if (depth < 5 && ts.isVariableDeclaration(declaration) && declaration.initializer && readsParameterOf(project, declaration.initializer, component, depth + 1, seen)) found = true;
      }
    }
    ts.forEachChild(child, visit);
  };
  visit(node);
  return found;
}

/** The expressions a result is about: the argument at `site`, or the arguments of the call at it. */
function flaggedArguments(site) {
  if (argumentSlot(site)) return [site];
  if (ts.isCallExpression(site) || ts.isNewExpression(site)) return [...(site.arguments ?? [])];
  return [site];
}

/** Where a member's declaration lives, how the member is used, from TypeScript's resolution of the access. */
function accessShape(project, access) {
  const name = ts.isPropertyAccessExpression(access) ? access.name : access;
  const symbol = ts.isIdentifier(name) ? project.checker.getSymbolAtLocation(name) : undefined;
  const declaration = symbol?.declarations?.[0];
  const where = declaration ? origin(declaration) : "unresolved";
  const type = project.checker.getNonNullableType(project.checker.getTypeAtLocation(access));
  const parts = type.isUnion() ? type.types : [type];
  const callable = parts.some((one) => one.getCallSignatures().length > 0);
  const onlyCallable = parts.every((one) => one.getCallSignatures().length > 0);
  const { node, parent } = peel(access);
  const called = Boolean(parent && ts.isCallExpression(parent) && parent.expression === node);
  // A member typed as a function or a plain value (`MaybeAccessor<T>`) is neither a method nor a callable read.
  return { where: where.startsWith("package") ? "package" : where, nature: called ? "method" : onlyCallable ? "callable-read" : callable ? "maybe-callable-read" : "data-read" };
}

/** The accesses of `path` from `reference`, one property after the other; null when it is not written so. */
function walkPath(reference, path) {
  let node = reference;
  for (const segment of path) {
    const { node: self, parent } = peel(node);
    if (parent && ts.isPropertyAccessExpression(parent) && parent.expression === self && parent.name.text === segment) node = parent;
    else if (parent && ts.isElementAccessExpression(parent) && parent.expression === self && ts.isStringLiteralLike(parent.argumentExpression) && parent.argumentExpression.text === segment) node = parent;
    else return null;
  }
  return node;
}

/**
 * The access of member `path` on parameter `index` of `fn`, written in its
 * body or in a project function it hands the parameter to whole (the
 * forwarding rule, ADR 0320); `hops` counts the functions it was followed
 * through. Null when no such access is written.
 */
function traceMember(project, fn, index, path, depth = 0, seen = new Set()) {
  const parameter = fn?.parameters?.[index];
  if (!parameter || parameter.dotDotDotToken || !fn.body || seen.has(fn)) return null;
  seen.add(fn);
  let references;
  let rest = path;
  if (ts.isIdentifier(parameter.name)) references = referencesIn(project, fn.body, parameter.name);
  else if (ts.isObjectBindingPattern(parameter.name)) {
    const slot = parameter.name.elements.find((element) => !element.dotDotDotToken && ts.isIdentifier(element.name) && (element.propertyName ?? element.name).getText() === path[0]);
    if (!slot) return null;
    references = referencesIn(project, fn.body, slot.name);
    rest = path.slice(1);
  } else return null;
  // Where the member is written in this body: a place it is called over one it is only read.
  const accesses = references.map((reference) => walkPath(reference, rest)).filter(Boolean);
  const calledAccess = accesses.find((access) => {
    const { node, parent } = peel(access);
    return parent && ts.isCallExpression(parent) && parent.expression === node;
  });
  if (accesses.length) return { access: calledAccess ?? accesses[0], hops: depth };
  for (const reference of references) {
    const slot = argumentSlot(reference);
    if (!slot || depth >= 6 || slot.call.arguments.slice(0, slot.index + 1).some(ts.isSpreadElement)) continue;
    const callee = resolveCallee(project, slot.call);
    if (callee.where !== "project" || !callee.fn || ts.isNewExpression(slot.call)) continue;
    if (ts.isPropertyAccessExpression(slot.call.expression) && !isNamespaceMember(project, slot.call.expression)) continue;
    const inner = traceMember(project, callee.fn, slot.index, rest, depth + 1, seen);
    if (inner) return inner;
  }
  return null;
}

/** The member path a message names (`invokes .children.find on`), or null. */
const messagePath = (message) => message.match(/invokes \.(\S+) on a caller-supplied value/)?.[1]?.split(".") ?? null;

/** The call a result is about: the call whose argument is the site, or the one named in the message. */
function resultCall(site, message) {
  const name = message.match(/^([\w$]+) invokes/)?.[1] ?? message.match(/^the runtime target of ([\w$.]+) /)?.[1]?.split(".").pop();
  if (ts.isCallExpression(site) && name && calleeName(site) === name) return { call: site, index: -1 };
  const slot = argumentSlot(site);
  if (slot) return slot;
  const call = callAt(site, name);
  return call ? { call, index: -1 } : null;
}


/** How the value at `node` comes to be, by TypeScript's resolution: a literal, a call's result and where the callee is declared, a parameter or a member of one, a binding. */
function valueOrigin(project, node, depth = 0) {
  const value = unparenthesized(node);
  if (isFunction(value)) return "function-literal";
  if (ts.isObjectLiteralExpression(value)) return "object-literal";
  if (ts.isArrayLiteralExpression(value)) return "array-literal";
  if (ts.isLiteralExpression(value) || value.kind === ts.SyntaxKind.TrueKeyword || value.kind === ts.SyntaxKind.FalseKeyword) return "primitive-literal";
  if (ts.isNewExpression(value)) return `new-${resolveCallee(project, value).where}`;
  if (ts.isAwaitExpression(value)) return valueOrigin(project, value.expression, depth + 1);
  if (ts.isCallExpression(value)) return `call-${resolveCallee(project, value).where}`;
  if (ts.isConditionalExpression(value) || ts.isBinaryExpression(value)) return "computed";
  let root = value;
  let member = false;
  while (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root) || ts.isNonNullExpression(root) || ts.isCallExpression(root)) {
    member = member || !ts.isNonNullExpression(root);
    root = root.expression;
  }
  if (!ts.isIdentifier(root)) return ts.SyntaxKind[value.kind];
  return declarationOrigin(project, project.symbol(root)?.declarations?.[0], member, depth);
}

/** How the value a declaration holds comes to be (`valueOrigin` of what it names). */
function declarationOrigin(project, declaration, member, depth) {
  if (!declaration) return member ? "member-unresolved" : "unresolved";
  const where = origin(declaration);
  if (where !== "project") return `${member ? "member-of-" : ""}${where.startsWith("package") ? "package" : "lib"}-binding`;
  let destructured = false;
  while (declaration && ts.isBindingElement(declaration)) {
    destructured = true;
    declaration = declaration.parent?.parent;
  }
  if (!declaration) return "binding";
  if (ts.isParameter(declaration)) return member ? "member-of-parameter" : "parameter";
  if (ts.isVariableDeclaration(declaration) && declaration.initializer && depth < 3) {
    const inner = valueOrigin(project, declaration.initializer, depth + 1);
    return `${member ? "member-of-" : ""}${destructured ? "destructured-" : ""}${inner}`;
  }
  if (ts.isFunctionDeclaration(declaration)) return "function-declaration";
  return member ? "member-of-binding" : "binding";
}
const unparenthesized = (node) => {
  while (ts.isParenthesizedExpression(node) || ts.isAsExpression(node) || ts.isNonNullExpression(node) || ts.isSatisfiesExpression(node) || ts.isTypeAssertionExpression(node)) node = node.expression;
  return node;
};

/** The expressions a function returns (an expression body, or the `return`s of its own body). */
function returnedExpressions(fn) {
  if (!fn.body) return [];
  if (!ts.isBlock(fn.body)) return [fn.body];
  const found = [];
  const visit = (node) => {
    if (isFunction(node) || ts.isClassLike(node)) return;
    if (ts.isReturnStatement(node) && node.expression) found.push(node.expression);
    ts.forEachChild(node, visit);
  };
  visit(fn.body);
  return found;
}

/** The value a property of an object literal holds, or null when the literal has no plain property of that name (a spread, a computed name, a missing one). */
function literalProperty(literal, name) {
  if (literal.properties.some((property) => ts.isSpreadAssignment(property))) return null;
  for (const property of literal.properties) {
    if (property.name && (ts.isIdentifier(property.name) || ts.isStringLiteralLike(property.name)) && property.name.text === name) {
      return ts.isPropertyAssignment(property) ? property.initializer : property;
    }
  }
  return null;
}

/**
 * The structure of an event handler read from a member that is not a member of
 * `props` (`workspace.undo`), by TypeScript's resolution of how the object
 * comes to be. A member of a project function's result is `factory-member`
 * when every return of that function is an object literal that holds the
 * member as a function written in the project, else the result's
 * `other-member:factory-returns-<origins>`; any other object takes its own
 * origin (`other-member:member-of-parameter`). The member must be a function the
 * literal writes (a method, or a name bound to a function in the project):
 * one the factory takes from another call (`other-member:factory-member-<origin>`)
 * is another construct.
 */
function handlerMemberCause(project, site) {
  const origin = valueOrigin(project, site);
  if (origin !== "member-of-call-project") return `other-member:${origin}`;
  const path = [];
  let root = unparenthesized(site);
  while (ts.isPropertyAccessExpression(root)) {
    path.unshift(root.name.text);
    root = unparenthesized(root.expression);
  }
  const unread = `other-member:${origin}`;
  if (!ts.isIdentifier(root)) return unread;
  let declaration = project.symbol(root)?.declarations?.[0];
  while (declaration && ts.isBindingElement(declaration)) declaration = declaration.parent?.parent;
  if (!declaration || !ts.isVariableDeclaration(declaration) || !declaration.initializer) return unread;
  let call = unparenthesized(declaration.initializer);
  if (ts.isAwaitExpression(call)) call = unparenthesized(call.expression);
  if (!ts.isCallExpression(call)) return unread;
  const callee = resolveCallee(project, call);
  if (!callee.fn) return unread;
  const returned = returnedExpressions(callee.fn).map(unparenthesized);
  if (!returned.length) return unread;
  // The member's value where the literal writes it: a method, or a name or expression whose origin is a function written in the project.
  const memberValue = (expression, names) => {
    if (!ts.isObjectLiteralExpression(expression)) return null;
    const property = literalProperty(expression, names[0]);
    if (!property) return null;
    if (names.length > 1) return memberValue(unparenthesized(ts.isShorthandPropertyAssignment(property) ? property.name : property), names.slice(1));
    if (ts.isMethodDeclaration(property)) return "function-literal";
    if (!ts.isShorthandPropertyAssignment(property)) return valueOrigin(project, property);
    const bound = project.checker.getShorthandAssignmentValueSymbol(property)?.declarations?.[0];
    return declarationOrigin(project, bound, false, 0);
  };
  const values = returned.map((expression) => memberValue(expression, path));
  if (values.every((value) => value === "function-literal" || value === "function-declaration")) return "factory-member";
  if (values.every((value) => value !== null)) return `other-member:factory-member-${[...new Set(values)].sort().join("+")}`;
  return `other-member:factory-returns-${[...new Set(returned.map((expression) => valueOrigin(project, expression)))].sort().join("+")}`;
}

/**
 * What the argument a project-declared member is called on (in the callee, by
 * `memberCause`) is, by TypeScript's resolution of how its value comes to be:
 * `member-of-call-project` (`kui.api`, a member of a project function's
 * result), `new-project` (an instance built with `new` of a project class),
 * a parameter, a literal, a package function's result, and so on.
 */
function projectTypeCause(project, site, message) {
  const slot = resultCall(site, message);
  const argument = slot && slot.index >= 0 ? slot.call.arguments[slot.index] : null;
  return argument ? valueOrigin(project, argument) : "no-argument";
}

/** What the callee `fn` resolved from a call is: its resolved body, or why there is none. */
function calleeBodyOrReason(project, call) {
  const callee = resolveCallee(project, call);
  if (callee.where === "unresolved") return { reason: "callee-unresolved" };
  if (callee.where !== "project") return { reason: `callee-${callee.package ? `package:${callee.package}` : callee.where}` };
  if (!callee.fn) return { reason: "callee-project-without-body" };
  return { fn: callee.fn, member: ts.isPropertyAccessExpression(call.expression) && !isNamespaceMember(project, call.expression) };
}

/**
 * What a result about a member called on a caller-supplied value rests on,
 * for the three labels the old reading gave several mechanisms
 * (`project-type`, `member-not-in-callee`, `callee-unresolved`): where the
 * access is written (in the callee, or in a function it hands the parameter
 * to), where the member is declared, and whether it is called, a callable that
 * is only read, or data.
 */
function memberCause(project, old, site, message) {
  const path = messagePath(message);
  const slot = resultCall(site, message);
  if (!path || !slot) return old;
  const callee = calleeBodyOrReason(project, slot.call);
  if (callee.reason) return `${callee.reason}`;
  const trace = slot.index >= 0 ? traceMember(project, callee.fn, slot.index, path) : null;
  if (!trace) return old;
  const shape = accessShape(project, trace.access);
  if (trace.hops) return `forwarded:${shape.where}-${shape.nature}`;
  // Written in the callee's own body: the label the access has when the callee resolves by name.
  const label = shape.where === "project" ? (shape.nature === "method" ? "project-type" : `project-${shape.nature}`) : { "lib-dom": "lib-dom", "lib-es": "lib-es", package: "package-type", unresolved: "unresolved" }[shape.where] ?? shape.where;
  return label;
}

/**
 * What the exported function a result is about is: a component, a class
 * method, an object method, a callback a call is given (directly or in an
 * options object; where the call is declared), or a function bound to a name.
 */
function exportedKind(project, site) {
  if (!isFunction(site)) return "not-a-function";
  if (isComponentLike(site)) return "component";
  if (ts.isMethodDeclaration(site) && ts.isClassLike(site.parent)) return "class-method";
  let node = site;
  let optionObject = false;
  for (let depth = 0; node && depth < 4; depth++) {
    const { node: self, parent } = peel(node);
    if (!parent) break;
    if (ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent) || ts.isShorthandPropertyAssignment(parent)) {
      optionObject = true;
      node = parent.parent;
      continue;
    }
    if (ts.isCallExpression(parent) && parent.arguments.includes(self)) {
      const callee = resolveCallee(project, parent);
      return `callback-${optionObject ? "option" : "argument"}:${callee.package ? `package:${callee.package}` : callee.where}`;
    }
    break;
  }
  if (ts.isConstructorDeclaration(site)) return "class-constructor";
  if (ts.isMethodDeclaration(site) || ts.isGetAccessorDeclaration(site) || ts.isSetAccessorDeclaration(site)) return "object-method";
  return ts.isFunctionDeclaration(site) ? "function-declaration" : "function-expression";
}

/** How the argument object literal at `site` holds `member`: a function it carries, a function it names, data. */
function literalMemberCause(project, site, member) {
  const argument = unparenthesized(site);
  if (!ts.isObjectLiteralExpression(argument)) return `non-literal-argument:${valueOrigin(project, argument)}`;
  const property = argument.properties.find((one) => one.name && one.name.getText() === member);
  if (!property) return argument.properties.some(ts.isSpreadAssignment) ? "member-from-spread" : "member-absent";
  if (ts.isMethodDeclaration(property) || ts.isGetAccessorDeclaration(property)) return "method";
  const value = ts.isPropertyAssignment(property) ? unparenthesized(property.initializer) : property.name;
  if (isFunction(value)) return "function-literal";
  const type = project.checker.getTypeAtLocation(value);
  const callable = (type.isUnion() ? type.types : [type]).some((one) => one.getCallSignatures().length > 0);
  const reference = ts.isShorthandPropertyAssignment(property) ? declarationOrigin(project, project.checker.getShorthandAssignmentValueSymbol(property)?.declarations?.[0], false, 0) : valueOrigin(project, value);
  // What the reference names: a function the program declares, a value a call (a signal, a hook) hands back, or what the caller supplies.
  const result = reference.match(/^(destructured-)?call-(\w+)$/);
  // A result of a project function whose type a package declares (a Solid `Setter`) is that package's function passed along, not a closure the project wrote.
  const memberType = project.checker.getNonNullableType(project.checker.getTypeAtLocation(value));
  const named = memberType.aliasSymbol ?? (memberType.symbol?.name === "__type" ? undefined : memberType.symbol);
  const typed = result && result[2] === "project" && named?.declarations?.[0] && origin(named.declarations[0]).startsWith("package") ? ":package-typed" : "";
  // A closure taken from a tuple a project function returns (`const [value, set] = createStore()`) is another reference than one from its object.
  const bound = ts.isShorthandPropertyAssignment(property) ? project.checker.getShorthandAssignmentValueSymbol(property)?.declarations?.[0] : ts.isIdentifier(value) ? project.symbol(value)?.declarations?.[0] : undefined;
  const tuple = result && result[2] === "project" && bound && ts.isBindingElement(bound) && ts.isArrayBindingPattern(bound.parent) ? "tuple-" : "";
  const kind = /^(function-declaration|function-literal)$/.test(reference) ? "declared-function" : result ? `call-result:${tuple}${result[1] ?? ""}${result[2]}${typed}` : /^(destructured-)?call-/.test(reference) ? "call-result" : "caller-supplied";
  return callable ? `function-reference:${kind}` : `data-${valueOrigin(project, value) === "primitive-literal" ? "literal" : "value"}`;
}

/** What the omitted default of the parameter a call leaves out is, and what the callee does with it. */
function defaultCause(project, site, message) {
  const slot = resultCall(site, message);
  if (!slot) return "no-call";
  const callee = calleeBodyOrReason(project, slot.call);
  if (callee.reason) return callee.reason;
  const given = slot.call.arguments?.length ?? 0;
  const parameter = slot.index >= 0 ? callee.fn.parameters[slot.index] : callee.fn.parameters.find((one, index) => index >= given && one.initializer);
  if (!parameter?.initializer) return "default-not-found";
  const initializer = valueOrigin(project, parameter.initializer);
  return `${slot.index >= 0 ? "explicit-undefined" : "omitted"}:${initializer}`;
}

/** What the function a result's callee name resolves to is, and how the callable it names came to be bound. */
function returnedCallableCause(project, site) {
  const target = ts.isPropertyAccessExpression(site) ? site.name : site;
  let declaration = project.symbol(target)?.declarations?.[0];
  // A name another name is bound to (`export const useTypeGPU = use`) is that name.
  for (let hops = 0; hops < 4 && declaration && ts.isVariableDeclaration(declaration) && declaration.initializer && ts.isIdentifier(unparenthesized(declaration.initializer)); hops++) {
    declaration = project.symbol(unparenthesized(declaration.initializer))?.declarations?.[0] ?? declaration;
  }
  if (!declaration) return "unresolved";
  const where = origin(declaration);
  const simple = where === "project" ? where : where.startsWith("package") ? "package" : "lib";
  if (simple !== "project") return `${simple}-function`;
  if (ts.isBindingElement(declaration)) {
    const pattern = declaration.parent;
    const holder = pattern.parent;
    const shape = ts.isArrayBindingPattern(pattern) ? "tuple-element" : "object-member";
    const init = ts.isVariableDeclaration(holder) ? holder.initializer && unparenthesized(holder.initializer) : null;
    return `${shape}-of-${init ? valueOrigin(project, init) : "unknown"}`;
  }
  if (ts.isVariableDeclaration(declaration)) return `bound-to-${declaration.initializer ? valueOrigin(project, declaration.initializer) : "nothing"}`;
  if (ts.isParameter(declaration)) return "parameter";
  if (ts.isFunctionDeclaration(declaration)) return "function-declaration";
  return ts.SyntaxKind[declaration.kind];
}

/** `createSignal(<one literal>)`: the dialect's signal with only literal data, which the module census allows before an object literal. */
function isDataSignal(project, call) {
  const callee = unparenthesized(call.expression);
  if (!ts.isIdentifier(callee) || call.arguments.length > 1) return false;
  const declaration = project.symbol(callee)?.declarations?.[0];
  if (!declaration || origin(declaration) !== "package:solid-js" || project.symbol(callee)?.name !== "createSignal") return false;
  return call.arguments.every((argument) => ts.isStringLiteralLike(argument) || ts.isNumericLiteral(argument) || argument.kind === ts.SyntaxKind.TrueKeyword || argument.kind === ts.SyntaxKind.FalseKeyword || argument.kind === ts.SyntaxKind.NullKeyword);
}

/**
 * Why the member call `receiver.member` at `site` stays unselected though its
 * receiver is an object literal binding (object_members.rs `receiver_closed`,
 * ADR 0299): the census of the object that proves a member has to hold for
 * every use of it, so one of these withholds every member: the module runs a
 * call that is no literal-valued signal before the literal; a use runs before
 * the literal is initialized (inside it, for an object a function holds); a
 * use is no call of a member (the object is handed on); a member some use
 * calls is no function literal free of `this`.
 */
function withheldCause(project, site) {
  const callee = unparenthesized(site);
  if (!ts.isPropertyAccessExpression(callee) || !ts.isIdentifier(callee.expression)) return "receiver-not-name";
  const symbol = project.symbol(callee.expression);
  const declaration = symbol?.declarations?.[0];
  if (!declaration || !ts.isVariableDeclaration(declaration) || !declaration.initializer) return "receiver-not-variable";
  let literal = unparenthesized(declaration.initializer);
  if (ts.isCallExpression(literal) && literal.arguments.length === 1) literal = unparenthesized(literal.arguments[0]);
  if (!ts.isObjectLiteralExpression(literal)) return `receiver-not-literal:${valueOrigin(project, declaration.initializer)}`;
  const holder = enclosingFunction(declaration);
  const reasons = new Set();
  if (!holder) {
    // Code the module runs before the literal: a call that is no literal-valued signal and no frozen literal.
    let early = false;
    const visit = (node) => {
      if (early || isFunction(node) || node.pos >= literal.pos) return;
      if (ts.isNewExpression(node) || (ts.isCallExpression(node) && !isDataSignal(project, node))) early = true;
      else ts.forEachChild(node, visit);
    };
    for (const statement of declaration.getSourceFile().statements) visit(statement);
    if (early) reasons.add("module-prefix-call");
  }
  const literalFunction = (name) => {
    const member = literal.properties.find((one) => one.name && one.name.getText() === name);
    if (!member) return false;
    const fn = ts.isMethodDeclaration(member) ? member : ts.isPropertyAssignment(member) ? unparenthesized(member.initializer) : null;
    return Boolean(fn && isFunction(fn) && !/\b(this|arguments|eval|super)\b/.test(fn.getText()));
  };
  for (const reference of project.referencesOf(symbol)) {
    const { node, parent } = peel(reference);
    if (parent && (ts.isExportSpecifier(parent) || ts.isImportSpecifier(parent) || ts.isImportClause(parent))) continue;
    const use = enclosingFunction(reference);
    const early = holder ? reference.getSourceFile() !== declaration.getSourceFile() || use !== holder || reference.getStart() < literal.end : !use;
    if (early) reasons.add("used-before-initialized");
    if (parent && ts.isPropertyAccessExpression(parent) && parent.expression === node) {
      const outer = peel(parent);
      const invoked = outer.parent && ((ts.isCallExpression(outer.parent) && outer.parent.expression === outer.node) || ts.isJsxExpression(outer.parent));
      if (!invoked) reasons.add("object-escapes");
      else if (!literalFunction(parent.name.text)) reasons.add("member-not-function-literal");
    } else reasons.add("object-escapes");
  }
  const order = ["module-prefix-call", "used-before-initialized", "object-escapes", "member-not-function-literal"];
  const found = order.filter((reason) => reasons.has(reason));
  return found.length ? found.join("+") : "census-open";
}

/** The unmodeled use of the argument at `site` in the function it is passed to; null when none is read. */
function unmodeledUse(project, site) {
  const slot = argumentSlot(site);
  if (!slot) return "no-argument";
  const callee = calleeBodyOrReason(project, slot.call);
  if (callee.reason) return callee.reason;
  return parameterUse(project, callee.fn, slot.index) ?? "no-use-read";
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
    const match = premise.match(/^synchronous dispatch \(ADR 0435\) at (.+):(\d+) withholds event handler/);
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


/**
 * The SC9012 veto labels that split on where the holding function runs. The
 * corpus attributes a result to the veto from the veto-off sweep (ADR 0447),
 * which does not say why that sweep certifies it; the veto withholds the
 * placement of an event handler, so `/handler` is a site the rest of the
 * reading places as one (`placementCause`), `/not-handler` any other.
 */
const VETO_SPLIT = new Set([
  "runtime-target-unselected/withheld-object-member/project-callback",
  "runtime-target-unselected/parameter-use-unmodeled/project-callback",
  "runtime-target-unselected/parameter-use-unmodeled/event-target+helper",
  "parameter-member-call/parameter-member-target-unresolved/event-target+helper",
  "parameter-member-call/parameter-member-path-unresolved/event-target+helper",
]);
const vetoSplits = (family, context, vetoed) => VETO_SPLIT.has(`${family}/${context}/${vetoed}`);

/**
 * The placement of the function holding an SC9012 site, by the rest of the
 * reading. A function handed to a project function is told apart by what binds
 * the callee: a parameter (`props.updateDocument(callback)`), a destructured
 * signal setter, a declared or imported function.
 */
function placementOf(project, fn, site) {
  const placement = placementCause(project, fn, site);
  if (placement !== "project-function-argument") return placement;
  const call = usesOf(project, fn).find((use) => use.kind === "argument")?.call;
  let root = call && unparenthesized(call.expression);
  let member = false;
  while (root && (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root))) {
    member = true;
    root = unparenthesized(root.expression);
  }
  const declaration = root && ts.isIdentifier(root) ? project.symbol(root)?.declarations?.[0] : null;
  const kind = !declaration ? "expression" : ts.isParameter(declaration) ? "parameter" : ts.isBindingElement(declaration) ? "destructured" : ts.SyntaxKind[declaration.kind];
  return `${placement}/${member ? "member-of-" : ""}${kind}`;
}

/** The veto label of an SC9012 result by the placement of the function holding it. */
const vetoSplit = (vetoed, placement) => `dispatch-veto:${vetoed}/${PLACED.test(placement) ? "handler" : `not-handler:${placement}`}`;

/** Set while the report classifies a veto-held SC9012 result: classify() records there whether its site is placed. */
let vetoProbe = null;

/**
 * Where the JSX element carrying the handler `fn` sits, up to the function
 * that holds it: in a conditional or logical expression (`cond && <button/>`,
 * `cond ? <a/> : <b/>`), else in a callback, else directly in the component.
 */
function handlerRegion(fn) {
  const { parent } = peel(fn);
  const attribute = parent && ts.isJsxExpression(parent) ? parent.parent : null;
  if (!attribute || !ts.isJsxAttribute(attribute)) return "not-a-literal-attribute";
  let node = attribute.parent.parent;
  for (let parent = node.parent; parent; node = parent, parent = parent.parent) {
    if (isFunction(parent)) return "in-callback";
    if (ts.isConditionalExpression(parent) && node !== parent.condition) return "in-conditional";
    if (ts.isBinaryExpression(parent) && [ts.SyntaxKind.AmpersandAmpersandToken, ts.SyntaxKind.BarBarToken, ts.SyntaxKind.QuestionQuestionToken].includes(parent.operatorToken.kind) && node === parent.right) return "in-conditional";
  }
  return "direct";
}

/** The structure segment of an SC9012 label: the mechanism the old reading's label stood for, split where it joined several. */
function sc9012Structure(project, finding, site, family, context, open) {
  const message = finding.message;
  if (family === "leaf-callback-body") return leafCalleeCause(project, site);
  if (family === "runtime-target-unselected" && context === "computed-call-target-unresolved") return computedTargetCause(project, site, enclosingFunction(site));
  if (family === "runtime-target-unselected" && context === "exported-parameter-member-dispatch") return exportedDispatchCause(project, isFunction(site) ? site : enclosingFunction(site));
  if (family === "dependency-source-getter" || family === "type-only-source") return placementCause(project, enclosingFunction(site), site);
  if (context === "parameter-member-target-unresolved" || context === "exported-parameter-member-dispatch") {
    if (context === "exported-parameter-member-dispatch" && family === "parameter-member-call") return exportedKind(project, site);
    const old = parameterMemberCause(project, site, message);
    const caused = family === "parameter-member-call" && ["project-type", "member-not-in-callee", "callee-unresolved"].includes(old) ? memberCause(project, old, site, message) : old;
    return caused === "project-type" && !open ? `${caused}/${projectTypeCause(project, site, message)}` : caused;
  }
  if (context === "parameter-use-unmodeled") {
    const kind = argumentKinds(project, site, message);
    if (kind === "typed-primitive-argument" && !open) {
      // A primitive proven by its type only is held by what the parameter does with it: wrapped in a literal that is handed on, or handed on whole.
      const use = unmodeledUse(project, site);
      return use.startsWith("wrapped:") ? `${kind}/wrapped/${use.slice("wrapped:".length)}` : `${kind}/${use}`;
    }
    return kind === "object-argument" ? `${kind}/${unmodeledUse(project, site)}` : kind;
  }
  if (family === "parameter-member-call" && context === "parameter-member-path-unresolved") return pathCause(project, site, message);
  if (family === "parameter-member-call" && context === "parameter-member-targets-diverge") {
    const member = messagePath(message)?.[0];
    return member ? literalMemberCause(project, site, member) : "";
  }
  if (context === "parameter-default-unresolved") return defaultCause(project, site, message);
  if (context === "withheld-object-member") return withheldCause(project, site);
  if (context === "returned-callable-outside-source-universe") return returnedCallableCause(project, site);
  if (context === "parameter-call-argument-unproven") return handlerArgumentCause(project, site);
  if (family === "primitive-value") return peel(site).parent ? ts.SyntaxKind[peel(site).parent.kind] : "no-parent";
  if (family === "package-callee") return packageCalleeCause(project, site);
  return "";
}

/** What binds the callable a call hands the argument at `site` to, by its declaration: a parameter, a `let` or `const`, a member of one. */
function packageCalleeCause(project, site) {
  const slot = argumentSlot(site);
  if (!slot) return "no-argument";
  let root = unparenthesized(slot.call.expression);
  let member = false;
  while (ts.isPropertyAccessExpression(root) || ts.isElementAccessExpression(root)) {
    member = true;
    root = unparenthesized(root.expression);
  }
  if (!ts.isIdentifier(root)) return "callee-expression";
  const declaration = project.symbol(root)?.declarations?.[0];
  let kind = declaration ? ts.SyntaxKind[declaration.kind] : "unresolved";
  if (declaration && ts.isParameter(declaration)) kind = "parameter";
  else if (declaration && ts.isVariableDeclaration(declaration)) kind = declaration.parent.flags & ts.NodeFlags.Const ? "const-binding" : "let-binding";
  return `${member ? "member-of-" : ""}${kind}`;
}

/** The member path result about a call that reaches a member through another member (`root.children.filter`). */
function pathCause(project, site, message) {
  const path = messagePath(message);
  const slot = resultCall(site, message);
  if (!path || !slot) return "untraced";
  const callee = calleeBodyOrReason(project, slot.call);
  if (callee.reason) return callee.reason;
  const trace = slot.index >= 0 ? traceMember(project, callee.fn, slot.index, path) : null;
  if (!trace) return "untraced";
  const shape = accessShape(project, trace.access);
  return `${shape.where}-${shape.nature}`;
}

/** What the argument is that a callee calls as a function (ADR 0338): the callee's own body is not the question, the argument's origin is. */
function handlerArgumentCause(project, site) {
  return argumentSlot(site) ? valueOrigin(project, site) : "no-argument";
}

/** The structures whose note (where a kept closure or a stored object goes) a case reads; the others keep the bare label. */
const NOTED_STRUCTURE = /^(object-member:argument-of:project\/called-in-kept-closure|project-function-argument\/(called-in-kept-closure|stored:object-member))@[^/]*$/;

/** The structures a case reads whose message names no enumeration of the component's callers. */
const CALLERS_CLAUSE_GATED = /^(object-member:argument-of:project\/called-in-kept-closure:result-member|project-function-argument\/(called-in-kept-closure:member-of:argument-of:project|stored:object-member:argument-of:project)|object-member:local-object:context-value(:nested-after-await)?)$/;

/** The cause of one uncertifiable finding. */
export function classify(project, finding, file, { vetoed, open = false }, entry = {}) {
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
  scope.rule = rule;
  scope.family = family;
  let structure;
  if (rule === "SC2001" || rule === "SC1001") {
    readSite = rule === "SC1001";
    try {
      if (family === "direct-read-callers-unenumerable") structure = `props-escape:${propsEscapeCause(project, fn, context, site)}`;
      else {
        structure = placementCause(project, fn, site);
        if (context === "condition") {
          // Only the condition variants a case stands for are read further.
          const plain = placementCause(project, fn, site, 0, new Set(), false);
          if (!CONDITION_STRUCTURES.has(plain)) structure = plain;
        }
      }
    } finally {
      readSite = false;
    }
    // A read nested in a callback of a method that has suspended: the method's await, not the callback's place, is what it waits on.
    if (rule === "SC1001" && /^object-member:local-object:context-value$/.test(structure)) {
      for (let outer = fn; outer; outer = enclosingFunction(outer)) {
        if (afterSuspension(outer, site)) {
          structure += ":nested-after-await";
          break;
        }
      }
    }
    if (rule === "SC1001" && family === "direct-read" && ["component-body", "package-callback"].includes(structure)) structure = `${structure}:${readInCalleeCause(project, finding)}`;
    if (rule === "SC1001" && family === "unplaced-read" && structure === "component-body") structure = `${structure}:${readPositionCause(site)}`;
    if (rule === "SC1001" && family === "unplaced-read" && context !== "condition" && structure === "after-await") structure = `after-await:${afterSuspension(fn, site) ? "in-async-function" : "promise-continuation"}`;
    // The note on a kept closure names the case's mechanism only where a case reads it.
    const noted = !open && !vetoed && family === "callback-to-unproven-consumer" && NOTED_STRUCTURE.test(structure);
    structure = structure.replace(/(called-in-kept-closure|read-in-kept-closure|stored:object-member)@([^/]*)/g, (match, label, note) => (noted ? `${label}:${note}` : label));
    // A prop read of a component whose callers cannot be enumerated carries a second obligation in its message; the cases below read the message without it.
    if (rule === "SC1001" && family === "callback-to-unproven-consumer" && !open && !vetoed && CALLERS_CLAUSE_GATED.test(structure) && /call sites cannot be enumerated/.test(finding.message)) structure += ":callers-unenumerable";
    // A handler the reading places that the checker still does not: where its element sits.
    if (rule === "SC1001" && family === "unplaced-read" && context !== "condition" && structure === "intrinsic-handler" && !vetoed) structure += `/${handlerRegion(fn)}`;
    // ADR 0356: while an unexcluded dispatch site remains, no handler is
    // placed; a site the rest of the reading places as a handler is held by
    // the veto alone.
    if (vetoed && PLACED.test(structure)) structure = `dispatch-veto:${vetoed}`;
    // A write's cause is the structure it sits in and where the checker's walk of the function's invocations stops.
    else if (rule === "SC2001" && (family === "unplaced-write" ? WALKED_STRUCTURES : family === "unplaced-call" ? WALKED_CALL_STRUCTURES : new Set()).has(structure)) {
      structure += `/${writeWalkReason(project, walkRoot(project, fn), structure)}`;
    }
  } else if (rule === "SC9012") {
    const escape = context.startsWith("parameter-") || context === "exported-parameter-member-dispatch" ? componentEscape(project, fn) : null;
    const component = escape ? enclosingComponent(fn) : null;
    if (escape && flaggedArguments(site).some((argument) => readsParameterOf(project, argument, component))) {
      structure = `escaped-component:${escape}`;
      // An argument that reaches the props only through a local binding is not a prop handed on.
      if (escape === "spread-caller" && context === "parameter-use-unmodeled" && !flaggedArguments(site).some((argument) => readsParameterOf(project, argument, component, 5))) structure += ":via-binding";
    }
    else if (vetoed && vetoSplits(family, context, vetoed)) structure = vetoSplit(vetoed, placementOf(project, fn, site));
    else structure = sc9012Structure(project, finding, site, family, context, open);
    if (vetoProbe) vetoProbe.placement = placementOf(project, fn, site);
  } else if (rule === "SC7007") {
    structure = family === "server-function-argument" ? serverArgumentCause(project, site) : "";
  } else if (rule === "SC1003" || rule === "SC1004" || rule === "SC5003") {
    structure = componentFlavorCause(project, fn);
    // Destructuring in the parameter list (`({ ...props })`) or in the body (`const { a } = props`).
    if (rule === "SC1003" && ts.isObjectBindingPattern(site) && ts.isParameter(site.parent)) structure += ":parameter-pattern";
    if (rule === "SC5003") structure = `${hasRenderRoot(project) ? "root-present" : "no-root"}:${structure}`;
  } else if (rule === "SC4001") {
    if (!open && family === "exported-function-callers") structure = exportedHolderCause(project, site);
    else if (family === "unowned-primitive") structure = unownedPrimitiveCause(project, site);
    else if (context === "handler-spread-unresolved") {
      const name = finding.message.match(/hands (.*?) to an element/s)?.[1] ?? "";
      structure = /^props$/.test(name) ? "props" : spreadOperand(project, site);
    } else if (context === "handler-member-unresolved") {
      const name = finding.message.match(/hands (.*?) to an element/s)?.[1] ?? "";
      structure = /^_?props\.[\w$]+$/.test(name) ? "props-member" : open ? "other-member" : handlerMemberCause(project, site);
    } else if (open && family === "exported-function-callers") {
      // A call that needs an owner (`useContext`, `onCleanup`), or a callback
      // whose returned cleanup needs one (`onSettled(() => ...)`).
      structure = isFunction(site) ? "cleanup-callback" : "call";
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
export function classifyReport(report, closedReport = null, noVetoReport = null) {
  const counts = (other) => {
    const held = new Map();
    const rows = new Set();
    const premises = new Map();
    for (const row of other?.results ?? []) {
      rows.add(`${row.app}|${row.project}`);
      premises.set(`${row.app}|${row.project}`, row.premises ?? []);
      for (const finding of row.findings) {
        if (finding.kind !== "uncertifiable") continue;
        const k = key(row, finding);
        held.set(k, (held.get(k) ?? 0) + 1);
      }
    }
    return { held, rows, premises };
  };
  const { held: closed, rows: closedApps, premises: closedPremises } = counts(closedReport);
  const { held: unvetoed, rows: unvetoedApps } = counts(noVetoReport);
  const classified = [];
  for (const row of report.results) {
    const findings = row.findings.filter((finding) => finding.kind === "uncertifiable");
    if (!findings.length) continue;
    const project = new Project(row.root, row.project);
    // A result the closed run keeps is held by what holds there: closing a
    // program can also exclude dispatch sites (ADR 0438's test-only helper).
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
        // The boundary is a counterfactual (whatever else is true of the
        // result), so the label keeps the result's own reading after it: the
        // message family, the context and the structure around the site, read
        // with no veto. A case claims only the readings it reproduces.
        const file = project.file(location.path);
        if (file) {
          // A site the rest of the reading places as a handler (ADR 0356's
          // reading, as for the veto) is held by the open program's veto.
          const reading = classify(project, finding, file, { vetoed: null, open: true }, entry).replace(/(^|\/)([^/]*[>:])?(intrinsic-handler|placed)(\/[^/]+)?$/, "$1handler-placed");
          entry.cause = reading.replace(finding.id, `${finding.id}/open-program-boundary`);
        } else entry.cause = `${finding.id}/open-program-boundary`;
      } else {
        if (closed.get(k) > 0) closed.set(k, closed.get(k) - 1);
        const file = project.file(location.path);
        const appKey = `${row.app}|${row.project}`;
        if (!file) entry.cause = `${finding.id}/file-not-in-program`;
        else if (vetoed && unvetoedApps.has(appKey)) {
          // With a sweep whose build ignores the veto, the veto holds exactly
          // the results that sweep no longer has; any other is read with no
          // veto, whatever its site looks like.
          if (unvetoed.get(k) > 0) {
            unvetoed.set(k, unvetoed.get(k) - 1);
            entry.cause = classify(project, finding, file, { vetoed: null }, entry);
          } else {
            vetoProbe = finding.id === "SC9012" && vetoSplits(entry.family, entry.context, vetoed) ? { placement: "" } : null;
            const reading = classify(project, finding, file, { vetoed: null }, entry);
            const probe = vetoProbe;
            vetoProbe = null;
            // The veto holds a site the rest of the reading places as a
            // handler. One it holds that the reading places elsewhere (a
            // handler a project factory returns, a helper) is another
            // construct: a story-held site keeps its reading apart.
            const handler = PLACED.test(reading);
            const veto = vetoed === "story" && !handler && ["SC1001", "SC2001"].includes(finding.id) ? `dispatch-veto-other:story/${reading.split("/").pop()}` : `dispatch-veto:${vetoed}`;
            entry.cause = [finding.id, entry.family, entry.context, probe ? vetoSplit(vetoed, probe.placement) : veto].filter((part) => part !== "").join("/");
          }
        } else entry.cause = classify(project, finding, file, { vetoed }, entry);
      }
      classified.push(entry);
    }
  }
  return classified;
}

/**
 * A reading that places the site as a handler. One segment may follow the
 * placement: what a placed prop is further handed to (`/handed-to-solid:merge`)
 * names its route, not a blocker.
 */
const PLACED = /(^|[>:/])(intrinsic-handler|placed)(\/[^/]+)?$/;

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
  const sitesPerCase = new Map();
  for (const entry of classified) {
    const owner = caseFor(entry.cause, cases);
    const sites = sitesPerCase.get(owner?.id ?? null) ?? new Set();
    sites.add(siteKey(entry));
    sitesPerCase.set(owner?.id ?? null, sites);
  }
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
  return { covered, perCase, sitesPerCase };
}

/**
 * A distinct source site: the app's file, line and rule. Several projects of
 * one app (a workspace's packages) that analyze one file give one site.
 */
export const siteKey = (site) => `${site.app}|${site.path}|${site.line}|${site.rule}`;

/**
 * The seeded sample of each case's distinct sites a source read verifies
 * (ADR 0446): the same sites for the same report, whoever draws them.
 */
export function drawSamples(sites, id, size) {
  const rank = (key) => createHash("sha256").update(`${id}|${key}`).digest("hex");
  return [...sites].sort((a, b) => (rank(a) < rank(b) ? -1 : 1)).slice(0, size);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const options = {
    report: null,
    "closed-report": null,
    "noveto-report": null,
    repro: null,
    cases: null,
    checker: join(repo, "rust/target/release/solid-checker-rust"),
    typefacts: join(repo, "bin/solid-typefacts"),
    json: null,
    samples: "3",
    cause: null,
    draw: null,
    "draw-out": null,
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
  const classified = classifyReport(report, closed, !options.repro && options["noveto-report"] ? read(options["noveto-report"]) : null);
  const { covered, perCase, sitesPerCase } = printGroups(classified, table.cases, Number(options.samples), options.cause);
  const problems = [];
  if (options.repro) problems.push(...caseProblems(table.cases, classified, report));
  else if (table.cases.length) {
    const allSites = new Set(classified.map(siteKey)).size;
    const uncertifiable = table.cases.filter((one) => one.kind === "uncertifiable");
    const verified = uncertifiable.reduce((sum, entry) => sum + (entry.verifiedWeight ?? 0), 0);
    const pct = (part, whole) => ((100 * part) / whole).toFixed(1);
    console.log(`\nlabel coverage: ${covered} of ${classified.length} results (${pct(covered, classified.length)}%) carry a case's label`);
    console.log(`verified coverage: ${verified} of ${allSites} distinct sites (${pct(verified, allSites)}%) are under a label whose sampled sites all read as its case`);
    const drawn = {};
    for (const entry of uncertifiable) {
      const weight = perCase.get(entry.id) ?? 0;
      const sites = sitesPerCase.get(entry.id) ?? new Set();
      if (weight !== entry.labelWeight) problems.push(`${entry.id}: labelWeight ${weight} in this report, ${entry.labelWeight} in the table`);
      if (sites.size !== entry.sites) problems.push(`${entry.id}: ${sites.size} distinct sites in this report, ${entry.sites} in the table`);
      for (const sample of entry.verification?.samples ?? []) {
        if (!sites.has(siteKey(sample))) problems.push(`${entry.id}: sample ${sample.app}/${sample.path}:${sample.line} ${sample.rule} is no longer under the case's label; draw and read again`);
      }
      drawn[entry.id] = drawSamples(sites, entry.id, Number(options.draw ?? 0));
    }
    if (options.draw) writeFileSync(resolve(options["draw-out"] ?? "samples.json"), `${JSON.stringify(drawn, null, 1)}\n`);
    problems.push(...sourceProblems(table.cases, classified));
  }
  if (options.json) writeFileSync(resolve(options.json), `${JSON.stringify(classified, null, 1)}\n`);
  for (const problem of problems) console.error(problem);
  if (problems.length) process.exit(1);
}
