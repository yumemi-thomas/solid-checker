// What exists, regenerated from bytes (surface.json, surface-state.json, surface.test.mjs). The fuzzer's shapes came from the same
// understanding as the checker's rules, so `.focus()` was missed because nothing forced anyone to ask about the other DOM methods. This
// file asks the bytes instead: the pinned Solid dists and typings (scripts/fuzz/install), the pinned compiler, and the pinned TypeScript
// libs (`lib.dom.d.ts` and the ES libs of packages/cli's typescript). A Solid bump, or a new DOM method in a TypeScript bump, then fails
// surface.test.mjs naming the row, which is the question nobody asked.
//
//   surfaceRows()   one key per row of the real surface (`kind/name`):
//     entry/<module>                 an entry of a package's `exports` map
//     export/<module>/<name>         a runtime export, from the first entry that holds that value
//     overload/<fn>#<n>              a signature of an overloaded public function
//     option/<fn>/<prop>             a member of an options or props object of a public function or component
//     callback/<fn>/<param>          a function-typed parameter of a public function
//     code/<CODE>                    a diagnostic the runtime declares (`DiagnosticCode`) or emits (`code: "X"`, every dev dist)
//     jsx/form/<form>                a JSX form of JSX_FORMS, compiled by the pinned compiler
//     jsx/event/<name>               a delegated event name (the runtime's `DelegatedEvents`)
//     host/<Interface>.<method>      a method of a DOM interface a program can reach
//     host/<Name>                    a global function or constructor that takes a callback
//     std/<Interface>.<method>       a method of a standard-library value carrier (`Array`, `Map`, `Object`, `Reflect`, ...)
//     lang/<SyntaxKind>              a syntax node or operator of TypeScript's own enum
//     spec/<operation>               an abstract operation that runs user code: no bytes list holds these (surface-forms.mjs)
//     product/<axis>/<a>*<b>         a required crossing (surface-forms.mjs PRODUCTS): a name row says "this name appears", and defects
//                                    live in name × position × state
//   stateSites()    every call or assignment in the rc.21 dev dists that changes tracking, owner or strict-window state.
//   compileForms()  what the pinned compiler does with each JSX form (accepted or refused, and the helpers it lowers to).
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { NODE } from "../lib/node-runtime.mjs";
import { ABSTRACT_OPERATIONS, JSX_FORMS, productRows } from "./surface-forms.mjs";

const repo = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const INSTALL = join(repo, "scripts/fuzz/install");
const modules = join(INSTALL, "node_modules");
const CLI = join(repo, "packages/cli");
const PACKAGES = ["solid-js", "@solidjs/signals", "@solidjs/web"];
/** Conditions of the fuzzer's own runs: a development browser build. */
const CONDITIONS = ["browser", "development"];

/** Every importable entry of the three packages: the `exports` maps, with `/package.json` and the `/types/*` map left out. */
export function packageEntries() {
  return PACKAGES.flatMap((name) => {
    const manifest = JSON.parse(readFileSync(join(modules, name, "package.json"), "utf8"));
    return Object.keys(manifest.exports).filter((key) => key !== "./package.json" && !key.includes("*")).map((key) => (key === "." ? name : `${name}/${key.slice(2)}`));
  });
}

// One child process: a dist's runtime exports are what `import()` gives under the fuzzer's conditions, and importing one needs them set.
const LIST_EXPORTS = `
const entries = JSON.parse(process.argv[1]);
const seen = new Map(), out = {};
for (const entry of entries) {
  try {
    const module = await import(entry);
    out[entry] = { names: Object.keys(module).sort().map((name) => {
      const value = module[name];
      // A primitive (a header name, a symbol) is the same row under one name, wherever it is exported.
      const identity = typeof value === "function" || (typeof value === "object" && value !== null) ? value : \`\${name}=\${String(value)}\`;
      const first = seen.get(identity);
      if (first === undefined) seen.set(identity, entry);
      return first === undefined ? name : { name, from: first };
    }) };
  } catch (error) { out[entry] = { failed: String(error.message).split("\\n")[0].slice(0, 160) }; }
}
process.stdout.write(JSON.stringify(out));
`;

/** Each entry's runtime exports: `{ names: [name | { name, from }] }` (`from`: an earlier entry holds the same value), or `{ failed }`. */
export function runtimeExports(entries = packageEntries()) {
  const run = spawnSync(NODE, [...CONDITIONS.map((one) => `--conditions=${one}`), "--input-type=module", "-e", LIST_EXPORTS, JSON.stringify(entries)], { cwd: INSTALL, encoding: "utf8", maxBuffer: 1 << 26 });
  if (run.status !== 0) throw new Error(`surface: listing exports failed: ${run.stderr.slice(0, 400)}`);
  return JSON.parse(run.stdout);
}

/** The pinned TypeScript, for its checker on the typings and its libs. */
export const typescript = () => createRequire(join(CLI, "package.json"))("typescript");

/** A checker over the public typings and the DOM and ES libs; `ts.createProgram` of these takes about a second. */
function typedSurface(ts) {
  const roots = [join(modules, "solid-js/types/index.d.ts"), join(modules, "@solidjs/web/types/index.d.ts"), join(modules, "@solidjs/signals/dist/types/index.d.ts")];
  const entryTypes = PACKAGES.flatMap((name) => Object.entries(JSON.parse(readFileSync(join(modules, name, "package.json"), "utf8")).exports).flatMap(([key, value]) => (typeof value === "object" && value.types && !key.includes("*") ? [join(modules, name, value.types)] : [])));
  const program = ts.createProgram([...new Set([...roots, ...entryTypes])], { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, lib: ["lib.esnext.d.ts", "lib.dom.d.ts"], types: [], noEmit: true, skipLibCheck: true, jsx: ts.JsxEmit.Preserve });
  return { program, checker: program.getTypeChecker(), roots };
}

const solidFile = (file) => file.includes("/node_modules/solid-js/") || file.includes("/node_modules/@solidjs/");
const fromSolid = (symbol) => (symbol.declarations ?? []).some((declaration) => solidFile(declaration.getSourceFile().fileName));
const isFunctionType = (type) => type.getCallSignatures().length > 0;

function apiRows(ts, { program, checker, roots }, details) {
  const rows = new Set();
  const done = new Set();
  for (const root of roots) {
    const moduleSymbol = checker.getSymbolAtLocation(program.getSourceFile(root));
    for (const exported of checker.getExportsOfModule(moduleSymbol)) {
      const symbol = exported.flags & ts.SymbolFlags.Alias ? checker.getAliasedSymbol(exported) : exported;
      const name = exported.name;
      if (done.has(name) || !(symbol.flags & ts.SymbolFlags.Value)) continue;
      done.add(name);
      const type = checker.getTypeOfSymbol(symbol);
      const signatures = type.getCallSignatures();
      if (signatures.length > 1) signatures.forEach((signature, index) => { rows.add(`overload/${name}#${index}`); details[`overload/${name}#${index}`] = checker.signatureToString(signature); });
      for (const signature of signatures) {
        for (const parameter of signature.getParameters()) {
          const parameterType = checker.getTypeOfSymbol(parameter);
          for (const part of parameterType.isUnion() ? parameterType.types : [parameterType]) {
            if (part.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Null)) continue;
            if (isFunctionType(part)) { rows.add(`callback/${name}/${parameter.name}`); continue; }
            if (!(part.flags & ts.TypeFlags.Object)) continue;
            for (const property of checker.getPropertiesOfType(part)) if (fromSolid(property) && !/^(__@|_)/.test(property.name) && !(property.declarations ?? []).some((one) => /\/jsx(-properties)?\.d\.ts$/.test(one.getSourceFile().fileName))) rows.add(`option/${name}/${property.name}`);
          }
        }
      }
    }
  }
  return [...rows];
}

// The DOM a program reaches: the interfaces of the elements it can make, the document and window, and what their callbacks hold.
const DOM_INTERFACES = ["EventTarget", "Node", "Element", "HTMLElement", "SVGElement", "ShadowRoot", "Document", "Window", "HTMLDialogElement", "HTMLFormElement", "HTMLInputElement", "HTMLTextAreaElement", "HTMLSelectElement", "HTMLButtonElement", "HTMLDetailsElement", "HTMLMediaElement", "HTMLAnchorElement", "HTMLImageElement", "HTMLIFrameElement", "HTMLCanvasElement", "HTMLTemplateElement", "HTMLSlotElement", "CustomElementRegistry", "AbortController", "AbortSignal", "Event", "History", "Location", "Storage", "Performance"];
// The standard library's value carriers and callback hosts: what a reactive value can pass through (the census's D3 family).
const STD_INSTANCE = ["Array", "ReadonlyArray", "Map", "Set", "WeakMap", "WeakSet", "WeakRef", "Promise", "FinalizationRegistry", "Generator", "AsyncGenerator", "Function", "Object", "Iterator"];
const STD_STATIC = ["ObjectConstructor", "ArrayConstructor", "PromiseConstructor", "ProxyConstructor", "Reflect", "JSON", "MapConstructor", "SetConstructor", "SymbolConstructor", "FunctionConstructor"];

const interfaceSymbol = (ts, checker, anchor, name) => checker.resolveName(name, anchor, ts.SymbolFlags.Type | ts.SymbolFlags.Namespace | ts.SymbolFlags.Value, false);

function membersOf(ts, checker, symbol) {
  const type = symbol.flags & ts.SymbolFlags.Interface || symbol.flags & ts.SymbolFlags.Class ? checker.getDeclaredTypeOfSymbol(symbol) : checker.getTypeOfSymbol(symbol);
  const anchor = symbol.declarations?.[0];
  const out = [];
  for (const property of checker.getPropertiesOfType(type)) {
    if (property.name.startsWith("__@")) continue;
    const owner = property.declarations?.[0]?.parent;
    const from = owner?.name?.text ?? symbol.name;
    // An `on*` handler property is one row per interface: a program assigns a closure to it, whichever event it names.
    if (!(property.flags & ts.SymbolFlags.Method)) {
      if (/^on[a-z]/.test(property.name) && isFunctionType(checker.getNonNullableType(checker.getTypeOfSymbolAtLocation(property, anchor)))) out.push(`${from}.on*`);
      else if (isFunctionType(checker.getTypeOfSymbolAtLocation(property, anchor))) out.push(`${from}.${property.name}`);
      continue;
    }
    // A subinterface's redeclared `addEventListener` is the one EventTarget method, not a row of each element.
    out.push(/^(add|remove)EventListener$/.test(property.name) ? `EventTarget.${property.name}` : `${from}.${property.name}`);
  }
  return out;
}

/** The names each entry's own typings export: a name outside them is a TS2305 in a program that imports it. */
function typedNames(ts, { checker, program }) {
  const out = {};
  for (const name of PACKAGES) {
    const manifest = JSON.parse(readFileSync(join(modules, name, "package.json"), "utf8"));
    for (const [key, value] of Object.entries(manifest.exports)) {
      const types = typeof value === "object" ? value.types : undefined;
      if (!types || key.includes("*") || key === "./package.json") continue;
      const file = program.getSourceFile(join(modules, name, types));
      const symbol = file && checker.getSymbolAtLocation(file);
      out[key === "." ? name : `${name}/${key.slice(2)}`] = symbol ? checker.getExportsOfModule(symbol).map((one) => one.name) : null;
    }
  }
  return out;
}

function hostRows(ts, { program, checker }) {
  const lib = program.getSourceFiles().find((file) => file.fileName.endsWith("/lib.dom.d.ts"));
  const rows = new Set();
  for (const name of DOM_INTERFACES) for (const member of membersOf(ts, checker, interfaceSymbol(ts, checker, lib, name))) rows.add(`host/${member}`);
  // Globals that take a callback: `declare function f(callback...)` and `declare var C: { new(callback...) }`.
  for (const statement of lib.statements) {
    if (ts.isFunctionDeclaration(statement) && statement.name && statement.parameters.some((one) => /^(callback|handler|fn)$/.test(one.name.getText(lib)) || /Callback|Handler/.test(one.type?.getText(lib) ?? ""))) rows.add(`host/${statement.name.text}`);
    if (ts.isVariableStatement(statement)) for (const declaration of statement.declarationList.declarations) {
      const signatures = checker.getTypeAtLocation(declaration.name).getConstructSignatures();
      if (signatures.some((signature) => signature.getParameters().some((parameter) => isFunctionType(checker.getNonNullableType(checker.getTypeOfSymbol(parameter)))))) rows.add(`host/${declaration.name.getText(lib)}`);
    }
  }
  return [...rows];
}

function stdRows(ts, { program, checker }) {
  const lib = program.getSourceFiles().find((file) => file.fileName.endsWith("/lib.es5.d.ts"));
  const rows = new Set();
  for (const name of [...STD_INSTANCE, ...STD_STATIC]) {
    const symbol = interfaceSymbol(ts, checker, lib, name);
    if (!symbol) throw new Error(`surface: no ${name} in the libs`);
    for (const member of membersOf(ts, checker, symbol)) rows.add(`std/${member}`);
  }
  return [...rows];
}

function languageRows(ts) {
  const kinds = ts.SyntaxKind;
  const named = Object.keys(kinds).filter((key) => Number.isNaN(Number(key)));
  // The enum's aliases (FirstStatement...) share a number with the real name: keep the name that is not a `First`/`Last` marker.
  const real = [...named.filter((key) => !/^(First|Last)/.test(key)), "ArrowFunction"];
  const syntax = real.filter((key) => /(Expression|Statement|Declaration|Literal|Element|Pattern|Assignment|Clause|Block|Member|Accessor|Constructor|Property|Method|Parameter|Decorator|Spread|Template|Span)$/.test(key) && !/^(JS|Jsx|Type|Interface|Module|Namespace|Import|Export|ExternalModule|Enum|Unparsed|Missing|Mapped|Named|Tuple|Index|Conditional|Infer|Constructor|Function|Keyword|Abstract|Asserts|Accessor|Modifier|Shorthand)?(Doc|TypeReference)/.test(key) && !/^JSDoc|^Jsx|Type$|TypeNode|Signature$|^NotEmitted|^Partially|^Synthetic|^Comma|^Bundle|^Source/.test(key));
  const operators = real.filter((key) => /(Token)$/.test(key) && /(Equals|Plus|Minus|Asterisk|Slash|Percent|Ampersand|Bar|Caret|Tilde|Exclamation|Question|LessThan|GreaterThan|Comma|DotDotDot)/.test(key) && !/^(Conflict|EndOf|JsxText|Slash(Asterisk)?Comment)/.test(key));
  return [...new Set([...syntax, ...operators])].map((key) => `lang/${key}`);
}

/** The diagnostic codes: `DiagnosticCode` in the typings, and every `code: "X"` of every dev dist, the `frames` client included. */
export function codeRows() {
  const typed = readFileSync(join(modules, "@solidjs/signals/dist/types/core/dev.d.ts"), "utf8");
  const union = typed.slice(typed.indexOf("type DiagnosticCode"));
  const declared = [...union.slice(0, union.indexOf(";")).matchAll(/"([A-Z][A-Z0-9_/]+)"/g)].map((match) => match[1]);
  const emitted = devDists().flatMap((file) => [...readFileSync(join(modules, file), "utf8").matchAll(/\bcode:\s*"([A-Z][A-Z0-9_/]+)"/g)].map((match) => match[1]));
  return [...new Set([...declared, ...emitted])].map((code) => `code/${code}`);
}

/** The development dists, client and server, of every entry. */
function devDists() {
  return ["@solidjs/signals/dist/dev-shared.js", "@solidjs/signals/dist/dev.js", "@solidjs/signals/dist/dev.attribution.js", "solid-js/dist/solid.dev.js", "solid-js/dist/server.dev.js", "solid-js/dist/refresh.dev.js", "solid-js/dist/attribution.js", "@solidjs/web/dist/web.dev.js", "@solidjs/web/dist/server.dev.js", "@solidjs/web/frames/dist/client.dev.js", "@solidjs/web/frames/dist/server.dev.js", "@solidjs/web/server-functions/dist/client.dev.js", "@solidjs/web/server-functions/dist/server.dev.js", "@solidjs/web/performance-tracks/dist/performance-tracks.dev.js"];
}

/** What the pinned compiler does with each form: `{ ok, helpers }`, or `{ ok: false, error }`. */
export function compileForms() {
  const { transform } = createRequire(join(INSTALL, "package.json"))("@solidjs/compiler");
  const out = {};
  for (const [form, source] of Object.entries(JSX_FORMS)) {
    try {
      const { code } = transform(`import { For, Show, Switch, Match, Loading, Errored, Reveal, Repeat, Portal, Dynamic, Hydration, NoHydration } from "solid-js";\nexport function App() { return (${source}); }`, { filename: "App.tsx", generate: "dom", dev: true, hydratable: false });
      const helpers = [...new Set([...code.matchAll(/import \{([^}]*)\} from "@solidjs\/web"/g)].flatMap((match) => match[1].split(",").map((one) => one.trim().split(" as ")[0]).filter(Boolean)))].sort();
      out[form] = { ok: true, helpers };
    } catch (error) { out[form] = { ok: false, error: String(error.message).split("\n")[0] }; }
  }
  return out;
}

/** The delegated event names, read from the runtime (`DelegatedEvents`). */
export function delegatedEvents() {
  const web = readFileSync(join(modules, "@solidjs/web/dist/web.dev.js"), "utf8");
  const start = web.indexOf("DelegatedEvents = /*#__PURE__*/ new Set(");
  const open = web.indexOf("[", start);
  return [...web.slice(open, web.indexOf("]", open)).matchAll(/"([a-z]+)"/g)].map((match) => match[1]);
}

/** Every row key of the real surface. */
export function surfaceRows() {
  const entries = packageEntries();
  const listed = runtimeExports(entries);
  const rows = new Set(entries.map((entry) => `entry/${entry}`));
  const notes = {};
  // `export/<entry>/<name>` -> every entry holding that value.
  const holders = {};
  for (const entry of entries) for (const one of listed[entry].names ?? []) if (typeof one !== "string") (holders[`export/${one.from}/${one.name}`] ??= [one.from]).push(entry);
  for (const entry of entries) {
    if (listed[entry].failed) { notes[entry] = listed[entry].failed; continue; }
    for (const one of listed[entry].names) {
      if (typeof one !== "string") continue;
      // jsx-runtime re-exports `@solidjs/web` from another build of it (two function values): its own names are the rows.
      if (/\/jsx(-dev)?-runtime$/.test(entry) && listed["@solidjs/web"].names.some((other) => (other.name ?? other) === one)) continue;
      rows.add(`export/${entry}/${one}`);
    }
  }
  const ts = typescript();
  const typed = typedSurface(ts);
  const details = {};
  for (const row of [...apiRows(ts, typed, details), ...hostRows(ts, typed), ...stdRows(ts, typed), ...languageRows(ts), ...codeRows(), ...productRows()]) rows.add(row);
  for (const form of Object.keys(JSX_FORMS)) rows.add(`jsx/form/${form}`);
  for (const event of delegatedEvents()) rows.add(`jsx/event/${event}`);
  for (const operation of ABSTRACT_OPERATIONS) rows.add(`spec/${operation}`);
  return { rows: [...rows].sort(), failedEntries: notes, details, typed: typedNames(ts, typed), holders };
}

// ---------------------------------------------------------------------------------------------------------------------
// The state table

/** The dists a client program's state changes live in: the two the review named, the signals core they call, and the `frames` client. */
export const STATE_DISTS = ["solid-js/dist/solid.dev.js", "@solidjs/web/dist/web.dev.js", "@solidjs/signals/dist/dev.js", "@solidjs/signals/dist/dev-shared.js", "@solidjs/web/frames/dist/client.dev.js"];
const STATE_CALLS = /(?<![.\w$#])(untrack|runWithOwner|flush|createRoot(?:\$1)?|setStrictRead|setPendingCheck|setSnapshotCapture|recompute|createMemo(?:\$1)?|createRenderEffect(?:\$1)?|createTrackedEffect(?:\$1)?)\(/g;
const STATE_ASSIGNMENTS = /(?<![.\w$#])(?<!(?:let|const|var) )(tracking|context|strictRead|strictPending)\s*=(?![=>])/g;
const stripComment = (line) => line.replace(/^\s*(\/\/|\*|\/\*).*$/, "").replace(/\/\/ .*$/, "");
const DECLARATION = /^(?:export\s+)?(?:async\s+)?(?:function\*?\s+([\w$]+)|(?:const|let|var)\s+([\w$]+)\s*=|class\s+([\w$]+))/;
// A `function name(` of the state primitives is the primitive itself, and a string or an error text is no site.
const isDefinition = (line, name) => new RegExp(`^\\s*(?:(?:export\\s+)?(?:async\\s+)?function\\*?\\s+)?${name.replace("$", "\\$")}\\([^)]*\\)\\s*\\{`).test(line);

/**
 * Every site: `{ key, file, line, primitive, label, enclosing, text }`. The key is the file, the enclosing top-level declaration, the
 * primitive and its ordinal there, so an edit elsewhere in a dist does not move it; `text` is the site's line, so a change to it fails.
 */
export function stateSites() {
  const sites = [];
  for (const file of STATE_DISTS) {
    const lines = readFileSync(join(modules, file), "utf8").split("\n");
    let enclosing = "(module)";
    const ordinals = new Map();
    lines.forEach((raw, index) => {
      const declared = raw.match(DECLARATION);
      if (declared) enclosing = declared[1] ?? declared[2] ?? declared[3];
      const line = stripComment(raw);
      if (!line.trim() || /^\s*["'`]/.test(line)) return;
      const hits = [...line.matchAll(STATE_CALLS), ...line.matchAll(STATE_ASSIGNMENTS)].sort((a, b) => a.index - b.index);
      for (const hit of hits) {
        const primitive = hit[1].replace("$1", "");
        if (isDefinition(line, hit[1])) continue;
        // Inside a string (an error message names `flush()`): an odd count of quotes before the hit.
        const before = line.slice(0, hit.index);
        if ((before.match(/"/g) ?? []).length % 2 === 1 || (before.match(/`/g) ?? []).length % 2 === 1) continue;
        const ordinalKey = `${enclosing}/${primitive}`;
        const ordinal = ordinals.get(ordinalKey) ?? 0;
        ordinals.set(ordinalKey, ordinal + 1);
        const short = file.split("/").pop();
        const label = primitive === "untrack" ? untrackLabel([line, ...lines.slice(index + 1, index + 8)].join(" "), hit.index) : undefined;
        sites.push({ key: `${short === "client.dev.js" ? "frames-client" : short}#${enclosing}/${primitive}#${ordinal}`, file, line: index + 1, primitive, ...(label === undefined ? {} : { label }), enclosing, text: raw.trim().slice(0, 160) });
      }
    });
  }
  return sites;
}

/** `untrack(fn, label)`: whether the call on this line passes a label, the argument that arms the strict window (`dev-shared.js` `untrack`). */
function untrackLabel(line, at) {
  let depth = 0, commas = 0, quote = null;
  for (let index = line.indexOf("(", at); index < line.length; index += 1) {
    const char = line[index];
    if (quote) { if (char === quote) quote = null; continue; }
    if (char === '"' || char === "`" || char === "'") { quote = char; continue; }
    if ("([{".includes(char)) depth += 1;
    if (")]}".includes(char)) { depth -= 1; if (depth === 0) return commas > 0; }
    if (char === "," && depth === 1) commas += 1;
  }
  return commas > 0;
}
