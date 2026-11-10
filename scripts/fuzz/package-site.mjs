// ADR 0957: the package site. Any misuse or legal program's site (the `use` node: the call or access the runtime reports under a code) can sit
// inside a dependency package instead of the program's own text, reached from App.tsx through one of the entries below. The checker reads a
// dependency's source (ADR 0284) but reports a finding in it only at the project call that runs it (ADRs 0290, 0943), so a package site is a
// different path from the same defect in the project, and a shape that is judged in the project is not judged in a package until it is generated
// there. This is the generic twin: `describeUse` prints the site as it does for the project (binding, prelude, module lines), and `packageSite`
// moves that text into an export of `fz-site`, whose parameters are the free names the text uses (the signals, a props object, a context), and
// puts the entry expression in the program's place. The runtime places a diagnostic at the authored frame below the package's own (the entry),
// as it does for `fz-lib` and `fz-ctx`; the site's own line is the entry's.
//
//   entry        what App.tsx writes                                  what the package exports
//   call         site1(a, b)                                          function site1(a, b)
//   call-inner   site1(a, b)                                          function site1 calling a package function that holds the site
//   dotcall      site1.call(null, a, b)                               function site1
//   reflect      Reflect.apply(site1, undefined, [a, b])              function site1
//   tagged       site1`${a}${b}`                                      function site1(strings, a, b)
//   new          new Site1(a, b)                                      class Site1, the site in its constructor
//   getter       make1(a, b).value                                    function make1 returning an object whose `value` getter holds the site
//   callback     apply1(site1, a, b)                                  function site1, handed by name to a project helper that calls it
//   jsx          <Site1 a={a} b={b} />                                component Site1 (the names are its props)
//   jsx-inner    <Site1 a={a} b={b} />                                component Site1 rendering `createComponent(Inner1, props)`: the hop inside the package
//   hop          createComponent(Site1, { a, b })                     component Site1
//   dynamic      render(() => <SiteDyn1 a={a} b={b} />, div), SiteDyn1 = dynamic(() => Site1): mounted, or the component never runs
//   lazy         render(() => <SiteLazy1 a={a} b={b} />, div), SiteLazy1 = lazy(() => import("fz-site")): the component is the default export
import { createRequire } from "node:module";
import * as nodeModule from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const SITE_PACKAGE = "fz-site";
/** The entries, in the order a grid crosses them. */
export const PKG_ENTRIES = ["call", "call-inner", "dotcall", "reflect", "tagged", "new", "getter", "callback", "jsx", "jsx-inner", "hop", "dynamic", "lazy"];
/** The entries that reach the export as a component (its props are the names), and so run the site in a component body of its own. */
export const COMPONENT_ENTRIES = ["jsx", "jsx-inner", "hop", "dynamic", "lazy"];
export const isPackageSite = (node) => node.k === "use" && Boolean(node.pkg);

/**
 * How the site gets its values. Handed (the default): the entry passes the project's signals, setters or props as arguments, so the package function is generic
 * in them and the checker, which reads a handed parameter as unresolved, can decline. Own (`<entry>-own`): the package creates the signals itself at its module
 * scope (a package's own signal, store or memo), so its text proves the access and only the attribution through the entry is in question: the shape the false cleans
 * of ADR 0957 are in. A signal is own-capable when its kind is one of these; the others (async, action, api kinds) are handed.
 */
export const OWN_KINDS = {
  undefined: (sig) => ({ lines: [`const [${sig.name}, ${sig.setter}] = createSignal(${sig.init ?? 0});`], uses: ["createSignal"] }),
  store: (sig) => ({ lines: [`const [${sig.name}, ${sig.setter}] = createStore({ v: ${sig.init ?? 0} });`], uses: ["createStore"] }),
  memo: (sig) => ({ lines: [`const ${sig.name} = createMemo(() => 1);`], uses: ["createMemo"] }),
};
export const ownCapable = (sigs) => sigs.length > 0 && sigs.every((sig) => typeof OWN_KINDS[String(sig.kind)] === "function" && /^[A-Za-z_$][\w$]*$/.test(sig.name) && /^[A-Za-z_$][\w$]*$/.test(sig.setter));
/** `"tagged-own"` -> `{ entry: "tagged", own: true }`. */
export const parsePkg = (value) => { const text = String(value); return text.endsWith("-own") ? { entry: text.slice(0, -4), own: true } : { entry: text, own: false }; };
/** Every shape name a grid crosses: the entries handed, and each again with its own values where the template has signals to own. */
export const pkgShapesOf = (sigs) => PKG_ENTRIES.flatMap((entry) => [entry, ...(ownCapable(sigs) ? [`${entry}-own`] : [])]);

const here = dirname(fileURLToPath(import.meta.url));
let loaded = null;
const typescript = () => (loaded ??= createRequire(join(resolve(here, "../../packages/cli"), "package.json"))("typescript"));

/** rc.21 declares these only in `solid-js/internal` (print.mjs `internal`). */
const INTERNAL = ["createErrorBoundary", "createLoadingBoundary", "createOwner"];
/** Names a site text may use that are no one's value: the language's and the page's globals. */
const GLOBALS = new Set(["undefined", "NaN", "Infinity", "arguments", "globalThis", "window", "document", "console", "Math", "Object", "Reflect", "Promise", "Array", "JSON", "String",
  "Number", "Boolean", "Symbol", "Date", "Error", "TypeError", "Event", "Set", "Map", "WeakMap", "WeakSet", "Proxy", "RegExp", "BigInt", "queueMicrotask", "setTimeout", "setInterval",
  "clearTimeout", "clearInterval", "structuredClone", "FormData", "HTMLElement", "getSelection", "requestAnimationFrame", "navigator", "location", "performance"]);

/**
 * Solid's own names a site's lines may use without an import of their own: the printer imports `createContext` for a template's `contextLine`, so the
 * line's text names it and `describeUse` does not. Such a name is imported in the package, never handed to it.
 */
const SOLID_API = new Set(["createSignal", "createMemo", "createEffect", "createRenderEffect", "createTrackedEffect", "createRoot", "createContext", "useContext", "createStore",
  "createProjection", "createOptimistic", "createOptimisticStore", "createReaction", "createDeferred", "createComponent", "onCleanup", "onSettled", "untrack", "latest", "isPending",
  "flush", "resolve", "until", "refresh", "action", "merge", "omit", "deep", "snapshot", "runWithOwner", "getOwner", "children", "lazy", "from", "reconcile", "Show", "For", "Switch",
  "Match", "Repeat", "Loading", "Errored", "Reveal"]);
const WEB_API = new Set(["render", "Portal", "dynamic", "Dynamic", "hydrate"]);

/** `node:module`'s type stripper (Node 22.13 and later), loaded by name so an older Node fails at the call, not at the import. */
function stripTypes(text) {
  if (typeof nodeModule.stripTypeScriptTypes !== "function") throw new Error("package-site: this Node has no module.stripTypeScriptTypes (22.13 or later)");
  return nodeModule.stripTypeScriptTypes(text, { mode: "strip" }).replace(/[ \t]+$/gm, "").replace(/\n{3,}/g, "\n\n");
}

/**
 * The names `text` (JavaScript with JSX) uses and does not declare, in the order they first appear: what the entry has to hand the export.
 * `bound` are names the caller declares around it (imports, the module lines moved with it). A declaration anywhere in the text binds its name
 * everywhere in it: the generated texts do not shadow one of their own free names.
 */
export function freeNames(text, bound = []) {
  const ts = typescript();
  const file = ts.createSourceFile("site.jsx", text, ts.ScriptTarget.Latest, true, ts.ScriptKind.JSX);
  const declared = new Set(bound);
  const referenced = [];
  const declare = (name) => {
    if (ts.isIdentifier(name)) declared.add(name.text);
    else if (ts.isObjectBindingPattern(name) || ts.isArrayBindingPattern(name)) for (const element of name.elements) if (ts.isBindingElement(element)) declare(element.name);
  };
  const visit = (node) => {
    if (ts.isVariableDeclaration(node) || ts.isParameter(node) || ts.isBindingElement(node)) declare(node.name);
    else if ((ts.isFunctionDeclaration(node) || ts.isFunctionExpression(node) || ts.isClassDeclaration(node) || ts.isClassExpression(node)) && node.name) declared.add(node.name.text);
    else if (ts.isCatchClause(node) && node.variableDeclaration) declare(node.variableDeclaration.name);
    else if (ts.isIdentifier(node)) {
      const parent = node.parent;
      const skipped = (ts.isPropertyAccessExpression(parent) && parent.name === node)
        || ((ts.isPropertyAssignment(parent) || ts.isMethodDeclaration(parent) || ts.isPropertyDeclaration(parent) || ts.isGetAccessorDeclaration(parent) || ts.isSetAccessorDeclaration(parent)) && parent.name === node)
        || (ts.isBindingElement(parent) && (parent.propertyName === node || parent.name === node))
        || (ts.isJsxAttribute(parent) && parent.name === node)
        || ((ts.isLabeledStatement(parent) || ts.isBreakOrContinueStatement(parent)) && parent.label === node)
        || (ts.isParameter(parent) && parent.name === node)
        || (ts.isVariableDeclaration(parent) && parent.name === node)
        || ((ts.isFunctionDeclaration(parent) || ts.isFunctionExpression(parent) || ts.isClassDeclaration(parent) || ts.isClassExpression(parent)) && parent.name === node)
        || (ts.isJsxClosingElement(parent) && parent.tagName === node);
      // An intrinsic element (`<p>`) names no value.
      const intrinsic = (ts.isJsxOpeningElement(parent) || ts.isJsxSelfClosingElement(parent)) && parent.tagName === node && /^[a-z]/.test(node.text);
      if (!skipped && !intrinsic) referenced.push(node.text);
    }
    ts.forEachChild(node, visit);
  };
  visit(file);
  return [...new Set(referenced)].filter((name) => !declared.has(name) && !GLOBALS.has(name));
}

const indent = (lines, by = "  ") => lines.map((line) => `${by}${line}`);
const joined = (names) => names.join(", ");

/**
 * The package's files and the entry for a site: `{ pkgFiles, text, solid, web, moduleLines, importLines }` over the base description of the site (`base`
 * is `describeUse`'s output for the same node with its own binding).
 * @param {object} base `describeUse`'s description: text, solid, web, namespaces, moduleLines, preludeLines, contextLine, importLines, files
 * @param {string} entry one of PKG_ENTRIES
 * @param {Record<string, string>} moduleNames `{ "solid-js": "S", "@solidjs/web": "W" }`: the namespace names
 */
export function packageSite(base, entry, moduleNames, { own = false, sigs = [] } = {}) {
  if (!PKG_ENTRIES.includes(entry)) throw new Error(`package-site: unknown entry ${entry}`);
  const strip = (line) => line.replaceAll("@@AUX@@", "");
  const prelude = base.preludeLines.map(strip);
  const moduleLines = [...(base.contextLine ? [base.contextLine] : []), ...base.moduleLines.map(strip)];
  const op = strip(base.text);
  const solid = [...new Set(base.solid)];
  const web = [...new Set(base.web)];
  const namespaces = [...new Set(base.namespaces)];
  // The names the imports bind, so the free-name pass does not ask the entry for them.
  const imported = [...solid, ...web].map((specifier) => specifier.split(/\s+as\s+/).at(-1)).concat(namespaces.map((module) => moduleNames[module]));
  const stripped = stripTypes([...moduleLines, ...prelude, `function __site() {\n${op};\n}`].join("\n"));
  const free = freeNames(stripped, imported);
  // Own values: a signal the text names is created at the package's module scope, and not handed.
  const owned = own ? sigs.filter((sig) => free.includes(sig.name) || free.includes(sig.setter)) : [];
  if (own && !owned.length) throw new Error("package-site: own values were asked for, and the site names no signal");
  const ownedNames = new Set(owned.flatMap((sig) => [sig.name, sig.setter]));
  const ownLines = owned.flatMap((sig) => OWN_KINDS[String(sig.kind)](sig).lines);
  const ownUses = owned.flatMap((sig) => OWN_KINDS[String(sig.kind)](sig).uses);
  const names = free.filter((name) => !SOLID_API.has(name) && !WEB_API.has(name) && !ownedNames.has(name));
  solid.push(...ownUses.filter((name) => !solid.includes(name)));
  // A Solid name the lines use that no specifier brought: imported here.
  solid.push(...free.filter((name) => SOLID_API.has(name) && !solid.includes(name)));
  web.push(...free.filter((name) => WEB_API.has(name) && !web.includes(name)));
  // The site, as a function body: the prelude lines (a binding's alias or wrapper), then the statement.
  const body = [...prelude, `${op};`];
  const component = COMPONENT_ENTRIES.includes(entry);
  const inner = entry === "call-inner" || entry === "jsx-inner";
  const param = component ? "__p" : null;
  const unpack = component && names.length ? [`const { ${joined(names)} } = __p;`] : [];
  const typedParams = names;
  const siteBody = (lines) => [...unpack, ...lines];

  const ts = [];
  const head = [];
  const internal = solid.filter((specifier) => INTERNAL.includes(specifier.split(/\s+as\s+/)[0]));
  const plain = solid.filter((specifier) => !internal.includes(specifier));
  const needs = new Set(plain);
  if (inner && component) needs.add("createComponent");
  if (needs.size) head.push(`import { ${[...needs].sort().join(", ")} } from "solid-js";`);
  if (internal.length) head.push(`import { ${internal.join(", ")} } from "solid-js/internal";`);
  if (web.length) head.push(`import { ${web.join(", ")} } from "@solidjs/web";`);
  for (const module of namespaces) head.push(`import * as ${moduleNames[module]} from "${module}";`);
  ts.push(...head, ...(head.length ? [""] : []), ...moduleLines, ...(moduleLines.length ? [""] : []), ...ownLines, ...(ownLines.length ? [""] : []));

  const dts = [];
  let call; const projectLines = []; const projectSolid = []; const projectWeb = []; let exportsDefault = false;
  switch (entry) {
    case "call": case "dotcall": case "reflect": case "callback": {
      ts.push(`export function site1(${joined(typedParams)}) {`, ...indent(body), "}");
      dts.push("export declare function site1(...args: any[]): any;");
      if (entry === "call") call = `site1(${joined(names)})`;
      if (entry === "dotcall") call = `site1.call(null${names.length ? `, ${joined(names)}` : ""})`;
      if (entry === "reflect") call = `Reflect.apply(site1, undefined, [${joined(names)}])`;
      if (entry === "callback") {
        projectLines.push("function apply1(f: (...args: any[]) => unknown, ...args: any[]) { return f(...args); }");
        call = `apply1(site1${names.length ? `, ${joined(names)}` : ""})`;
      }
      break;
    }
    case "call-inner":
      ts.push(`function inner1(${joined(typedParams)}) {`, ...indent(body), "}", "", `export function site1(${joined(typedParams)}) {`, `  return inner1(${joined(names)});`, "}");
      dts.push("export declare function site1(...args: any[]): any;");
      call = `site1(${joined(names)})`;
      break;
    case "tagged":
      ts.push(`export function site1(strings${typedParams.length ? `, ${joined(typedParams)}` : ""}) {`, ...indent(body), "}");
      dts.push("export declare function site1(strings: TemplateStringsArray, ...values: any[]): any;");
      call = `site1\`${names.length ? names.map((name) => `\${${name}}`).join("") : "x"}\``;
      break;
    case "new":
      ts.push("export class Site1 {", `  constructor(${joined(typedParams)}) {`, ...indent(body, "    "), "  }", "}");
      dts.push("export declare class Site1 {", "  constructor(...args: any[]);", "}");
      call = `new Site1(${joined(names)})`;
      break;
    case "getter":
      ts.push(`export function make1(${joined(typedParams)}) {`, "  return {", "    get value() {", ...indent(body, "      "), "      return 1;", "    },", "  };", "}");
      dts.push("export declare function make1(...args: any[]): { readonly value: number };");
      call = `make1(${joined(names)}).value`;
      break;
    default: {
      // The components: `__p` is the props object, the names its keys.
      const attributes = names.map((name) => `${name}={${name}}`).join(" ");
      const element = (tag) => `<${tag}${attributes ? ` ${attributes}` : ""} />`;
      if (inner) {
        ts.push(`function Inner1(${param}) {`, ...indent(siteBody(body)), "  return null;", "}", "", `export function Site1(${param}) {`, "  return createComponent(Inner1, __p);", "}");
      } else {
        ts.push(`export function Site1(${param}) {`, ...indent(siteBody(body)), "  return null;", "}");
      }
      if (entry === "lazy") { ts.push("", "export default Site1;"); exportsDefault = true; }
      dts.push("export declare function Site1(props: any): any;", ...(exportsDefault ? ["export default Site1;"] : []));
      if (entry === "hop") { projectSolid.push("createComponent"); call = `createComponent(Site1, { ${joined(names)} })`; }
      // `dynamic` and `lazy` render their component from an effect or a later task, so the element is mounted (`render` into a detached div): an element a
      // statement discards never runs the component (measured: `not-fired` for all 14 programs of the first cell).
      else if (entry === "dynamic") { projectWeb.push("dynamic", "render"); projectLines.push("const SiteDyn1 = dynamic(() => Site1);"); call = `render(() => ${element("SiteDyn1")}, document.createElement("div"))`; }
      else if (entry === "lazy") { projectSolid.push("lazy"); projectWeb.push("render"); projectLines.push(`const SiteLazy1 = lazy(() => import("${SITE_PACKAGE}"));`); call = `render(() => ${element("SiteLazy1")}, document.createElement("div"))`; }
      else call = element("Site1");
    }
  }
  const exportName = entry === "new" || COMPONENT_ENTRIES.includes(entry) ? "Site1" : entry === "getter" ? "make1" : "site1";
  const files = {
    "package.json": `${JSON.stringify({
      name: SITE_PACKAGE, version: "1.0.0", type: "module",
      peerDependencies: { "solid-js": "^2.0.0-rc.21" },
      exports: { ".": { types: "./index.d.ts", default: "./index.js" } },
    }, null, 2)}\n`,
    "index.js": `${stripTypes(ts.join("\n")).replace(/\n+$/, "")}\n`,
    "index.d.ts": `${dts.join("\n")}\n`,
  };
  return {
    pkgFiles: { [SITE_PACKAGE]: files },
    text: call,
    solid: projectSolid, web: projectWeb, namespaces: [],
    moduleLines: projectLines,
    // `lazy` imports the package where it is used, so the static import is not written. The project's own imports of the site's text (a binding through a
    // re-exporting `solid-api.ts`, a value from `helpers.ts`) and its files stay in the project: the names they bind are free names, handed to the export.
    importLines: [...(entry === "lazy" ? [] : [`import { ${exportName} } from "${SITE_PACKAGE}";`]), ...(base.importLines ?? [])],
    files: base.files,
    names, exportName,
  };
}
