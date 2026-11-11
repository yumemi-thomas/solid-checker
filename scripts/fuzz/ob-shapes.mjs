// The object-boundary shapes (ADR 0963): the site reached through a construct the host's callback runs for the code's own text, and its twin that never runs it.
//
// Sixty-four false cleans Chrome confirmed on 9e5a45080 (the researcher's, batch 10b) share one cause: the walk follows a call, and the site was run by something that is
// not spelled as one. Each construct below is a function the language runs by itself (a getter, a coercion, an iterator step, a Proxy trap, a constructor, a field
// initializer, a tag) or a helper another file's export reaches; the host's callback only triggers it.
//
//   ob-<construct>-<host>          misuse   the site is the body of the construct, and the host's callback triggers it
//   ob-<construct>-<host>-never    legal    the same text, the construct never triggered (the member never read, the class never constructed, the iterator never stepped,
//                                           the coercion never made, the helper never called): no site runs, so Chrome reports nothing, and any violation is one the
//                                           checker proves for code that never runs
//
//   family A, hosts of SC1002 (`memo-async`, `effect-async`: the construct is triggered after the compute's `await`) and of SC3001 (`settled`, `tracked`: the two leaf hosts)
//     getter, setter                                  `{ get v() { site } }` read, `{ set v(x) { site } }` assigned
//     tostring, valueof, toprimitive, tojson          a coercion: a template, a unary plus, `JSON.stringify`
//     iter-for, iter-spread, iter-from                the site is a user `[Symbol.iterator]()` method, consumed by `for…of`, spread, `Array.from`
//     next-for, next-spread, next-from                the site is the `next()` of the iterator it returns, consumed the same three ways
//     proxy-get, -set, -has, -delete, -keys, -apply, -construct    a Proxy trap, triggered by the operation that reaches it
//     ctor-new, ctor-super, ctor-implicit, super-method, fn-new     `new C`, a subclass's `super()`, its implicit super, `super.m()`, a function only ever called with `new`
//     field-direct, field-iife, field-helper, field-super          a class field's initializer: the site itself, an IIFE, a helper's call, the superclass's field
//     tagged                                                       the tag function of a tagged template
//
//   family C, host `apply` (SC6002, SC2008: an effect's apply): the site is the body of an exported helper
//     exp-fn-same, exp-const-same        `export function h` / `export const h = () =>`, called by the apply in the file
//     exp-fn-import, exp-const-import    the same, exported under another name by a specifier (`export { hx as h }`) and imported by that name from the module itself
//     exp-fn-alias, exp-const-alias      the same, imported from the module itself under an alias
//     exp-fn-reexport, exp-const-reexport the same, imported under an alias from a second module that re-exports it (`lib.ts`)
//     local                              not exported (the control: the walk follows a local helper)
//   The helper stays in App.tsx: a site is judged where the printer marks it, and the printer marks one file. The import forms reach it through the module itself (ADR 0934
//   `export-namespace-call` does the same), so what is measured is the call through an import of an export, not a helper in a second file.
//
// Decorators and `accessor` fields are not here: the pinned toolchain (tsrs, Oxc) does not take them (the batch-10b brief).
//
// Recall contexts only, a few base shapes (`obShape`): the question is how the construct is entered, not the spelling of the site. The census does not cross them with other
// codes (matrix.mjs CENSUS_SKIPS): they are the cells of the codes that name them.
import { contain, decl, domExpr, expr, fn, num, prim, ret } from "./model.mjs";

/**
 * `{ [construct]: (n) => spec }`: the text of each family A construct (model.mjs FRAME_CONTAINER_KINDS). `n(prefix)` is a fresh name; `run` triggers the construct, `idle` only keeps
 * its names used (a twin).
 */
const object = (member, after, run) => (n) => { const o = n("o"); return { head: `const ${o} = { ${member} `, close: " };", after, run: run(o), idle: [`void ${o};`] }; };
const iterable = (member, close, after, consume) => (n) => { const o = n("o"); return { head: `const ${o}: Iterable<number> = { ${member} `, close, after, run: consume(o), idle: [`void ${o};`] }; };
const proxy = (target, trap, after, run) => (n) => { const p = n("px"); return { head: `const ${p} = new Proxy(${target}, { ${trap} `, close: " });", after, run: run(p), idle: [`void ${p};`] }; };
const MAP = "{} as Record<string, number>";
/** A class whose constructor or method holds the site; `rest` is what follows it (a subclass), `last` the class the program uses, `run` what triggers it. */
const klass = (header, rest, run) => (n) => { const a = n("K"); const b = rest ? n("K") : a; return { head: `class ${a} {\n  ${header} `, close: `\n}${rest ? rest(a, b) : ""}`, nest: 1, run: run(b), idle: [`void ${b};`] }; };
/** A class field's initializer (`last` is the class `new` is called on: the subclass, when there is one). */
const field = (frame, last = false) => (n) => { const a = n("K"); const b = last ? n("K") : a; const h = n("fh"); return { ...frame(a, b, h), run: [`new ${b}();`], idle: [`void ${b};`] }; };
const CONSUME = { for: (o) => [`for (const _ of ${o}) {}`], spread: (o) => [`void [...${o}];`], from: (o) => [`void Array.from(${o});`] };

export const OB_CONSTRUCTS_A = {
  getter: object("get v()", ["return 1;"], (o) => [`void ${o}.v;`]),
  setter: object("set v(_x: number)", [], (o) => [`${o}.v = 1;`]),
  tostring: object("toString()", ['return "x";'], (o) => ["void `${" + o + "}`;"]),
  valueof: object("valueOf()", ["return 1;"], (o) => [`void (+${o});`]),
  toprimitive: object("[Symbol.toPrimitive]()", ["return 1;"], (o) => [`void (+${o});`]),
  tojson: object("toJSON()", ["return 1;"], (o) => [`void JSON.stringify(${o});`]),
  ...Object.fromEntries(Object.entries(CONSUME).map(([how, consume]) => [`iter-${how}`, iterable("[Symbol.iterator]()", " };", ["return [1][Symbol.iterator]();"], consume)])),
  ...Object.fromEntries(Object.entries(CONSUME).map(([how, consume]) => [`next-${how}`, iterable("[Symbol.iterator]() { return { next()", " }; } };", ["return { done: true as const, value: undefined };"], consume)])),
  "proxy-get": proxy(MAP, "get(_t, _k)", ["return 1;"], (p) => [`void ${p}.x;`]),
  "proxy-set": proxy(MAP, "set(_t, _k, _v)", ["return true;"], (p) => [`${p}.x = 1;`]),
  "proxy-has": proxy(MAP, "has(_t, _k)", ["return true;"], (p) => [`void ("x" in ${p});`]),
  "proxy-delete": proxy(MAP, "deleteProperty(_t, _k)", ["return true;"], (p) => [`delete ${p}.x;`]),
  "proxy-keys": proxy(MAP, "ownKeys(_t)", ["return [];"], (p) => [`void Object.keys(${p});`]),
  "proxy-apply": proxy("(() => 1) as () => number", "apply(_t, _this, _a)", ["return 1;"], (p) => [`void ${p}();`]),
  "proxy-construct": proxy("class {} as new () => object", "construct(_t, _a)", ["return {};"], (p) => [`void new ${p}();`]),
  "ctor-new": klass("constructor()", null, (a) => [`new ${a}();`]),
  "ctor-super": klass("constructor()", (a, b) => `\nclass ${b} extends ${a} {\n  constructor() {\n    super();\n  }\n}`, (b) => [`new ${b}();`]),
  "ctor-implicit": klass("constructor()", (a, b) => `\nclass ${b} extends ${a} {}`, (b) => [`new ${b}();`]),
  "super-method": klass("m()", (a, b) => `\nclass ${b} extends ${a} {\n  m() {\n    super.m();\n  }\n}`, (b) => [`new ${b}().m();`]),
  "fn-new": (n) => { const f = n("F"); return { head: `function ${f}(this: unknown) `, close: "", run: [`new (${f} as unknown as new () => object)();`], idle: [`void ${f};`] }; },
  "field-direct": field((a) => ({ mode: "expr", head: `class ${a} {\n  f = `, close: ";\n}" })),
  "field-iife": field((a) => ({ head: `class ${a} {\n  f = (() => `, close: ")();\n}", nest: 1, after: ["return 1;"] })),
  "field-helper": field((a, _b, h) => ({ head: `const ${h} = () => `, close: `;\nclass ${a} {\n  f = ${h}();\n}`, after: ["return 1;"] })),
  "field-super": field((a, b) => ({ head: `class ${a} {\n  f = (() => `, close: `)();\n}\nclass ${b} extends ${a} {}`, nest: 1, after: ["return 1;"] }), true),
  tagged: (n) => { const t = n("t"); return { head: `const ${t} = (_s: TemplateStringsArray) => `, close: ";", after: ["return 1;"], run: ["void " + t + "`x`;"], idle: [`void ${t};`] }; },
};
export const OB_CONSTRUCT_NAMES_A = Object.keys(OB_CONSTRUCTS_A);

/** The family C forms: how the apply reaches the exported helper. */
export const OB_CONSTRUCT_NAMES_C = ["exp-fn-same", "exp-fn-import", "exp-fn-alias", "exp-fn-reexport", "exp-const-same", "exp-const-import", "exp-const-alias", "exp-const-reexport", "local"];

/** The host each code is placed in, and the construct family the host takes. */
export const OB_HOSTS = { "memo-async": "A", "effect-async": "A", settled: "A", tracked: "A", apply: "C" };
/** The hosts of each code: SC1002's two async computes, SC3001's two leaves, the apply of SC6002 and SC2008. */
export const OB_CODES = {
  UNTRACKED_READ_AFTER_AWAIT: ["memo-async", "effect-async"],
  CLEANUP_IN_FORBIDDEN_SCOPE: ["settled", "tracked"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["settled", "tracked"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["settled", "tracked"],
  PRIMITIVE_IN_EFFECT_CALLBACK: ["apply"],
  FLUSH_IN_EFFECT_CALLBACK: ["apply"],
};

const constructsOf = (host) => (OB_HOSTS[host] === "A" ? OB_CONSTRUCT_NAMES_A : OB_CONSTRUCT_NAMES_C);
const contextsOf = (host) => constructsOf(host).flatMap((construct) => [`ob-${construct}-${host}`, `ob-${construct}-${host}-never`]);
/** `{ [context]: "misuse" | "legal" }`. */
export const OB_CONTEXTS = Object.fromEntries(Object.keys(OB_HOSTS).flatMap((host) => contextsOf(host).map((context) => [context, context.endsWith("-never") ? "legal" : "misuse"])));
export const OB_CONTEXT_NAMES = Object.keys(OB_CONTEXTS);

const NAME = new RegExp(`^ob-(${[...OB_CONSTRUCT_NAMES_A, ...OB_CONSTRUCT_NAMES_C].sort((a, b) => b.length - a.length).join("|")})-(${Object.keys(OB_HOSTS).join("|")})(-never)?$`);
/** `{ construct, host, never, family }` of an `ob-*` context name, or null. */
export const obParts = (context) => {
  const match = NAME.exec(context);
  if (!match || !constructsOf(match[2]).includes(match[1])) return null;
  return { construct: match[1], host: match[2], never: Boolean(match[3]), family: OB_HOSTS[match[2]] };
};

/**
 * The base shapes an `ob-*` cell takes. Family A: the direct binding with no layer, an `iife` or a `helper` around the site, and the `alias` binding (each prints inside the
 * component, where the host's callback and the construct are). Family C: the direct binding, bare or in an `iife`: the helper is a module-scope function, which sees none of a
 * component body's aliases or helpers (ADR 0934).
 */
export function obShape(context, shape) {
  const parts = obParts(context);
  if (!parts) return true;
  const layers = parts.family === "A" ? ["none", "iife", "helper"] : ["none", "iife"];
  return (shape.binding === "direct" && layers.includes(shape.layer ?? "none")) || (parts.family === "A" && shape.binding === "alias" && (shape.layer ?? "none") === "none");
}

/** The construct's frame around `stmts`: a misuse cell triggers it, its twin keeps the names used and runs nothing. */
function frameOf(construct, never, stmts, kit) {
  const spec = OB_CONSTRUCTS_A[construct](kit.name);
  if (spec.mode === "expr" && !(stmts.length === 1 && stmts[0].k === "expr")) throw new Error(`ob-${construct}: an initializer is one expression, got ${stmts.length} statements`);
  const { run, idle, ...frame } = spec;
  return contain("frame", 9, stmts, { ...frame, then: never ? idle : run });
}

/** Family C: `{ hoisted, importLines?, files?, called }` for a form: the helper at module scope, how it is named where the apply calls it. */
function helperOf(construct, stmts, kit) {
  const [, kind, how] = /^exp-(fn|const)-(same|import|alias|reexport)$/.exec(construct) ?? [];
  if (construct === "local") { const h = kit.name("hx"); return { hoisted: [decl(h, fn(stmts, { form: "function" }))], called: h }; }
  const h = kit.name("hx");
  const declared = kind === "fn" ? decl(h, fn(stmts, { form: "function" }), { exported: true }) : decl(h, fn(stmts), { kw: "export const" });
  if (how === "same") return { hoisted: [declared], called: h };
  const local = kit.name("hl");
  if (how === "alias") return { hoisted: [declared], importLines: [`import { ${h} as ${local} } from "./App";`], called: local };
  if (how === "reexport") return { hoisted: [declared], importLines: [`import { ${h} as ${local} } from "./lib";`], files: { "lib.ts": `export { ${h} } from "./App";\n` }, called: local };
  // `import`: the module's own export under the name a specifier gives it, imported by that name.
  const named = kit.name("hn");
  const plain = kind === "fn" ? decl(h, fn(stmts, { form: "function" })) : decl(h, fn(stmts));
  return { hoisted: [plain, expr(domExpr(`export { ${h} as ${named} }`))], importLines: [`import { ${named} } from "./App";`], called: named };
}

/**
 * The placement of `stmts` for an `ob-*` context (`{ body, view, ... }`), or undefined for any other. `place(host, inner)` is the plain host's own placement (misuse.mjs
 * `inContext` of `memo-async`, `effect-async`, `settled`, `tracked`, `effect-apply`).
 */
export function obContext(context, stmts, kit, place) {
  const parts = obParts(context);
  if (!parts) return undefined;
  if (parts.family === "A") return place(parts.host, [frameOf(parts.construct, parts.never, stmts, kit)]);
  const { hoisted, importLines, files, called } = helperOf(parts.construct, stmts, kit);
  // Raw text, as ADR 0934's `self[…]()`: the helper is a module-scope name the component's own scope check does not see.
  const apply = [expr(domExpr(parts.never ? `void ${called}` : `${called}()`))];
  return { body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn(apply) }))], view: [], hoisted, ...(importLines ? { importLines, files: files ?? {} } : files ? { files } : {}) };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function obPlacement() {
  return Object.fromEntries(Object.entries(OB_CODES).map(([code, hosts]) => {
    const contexts = hosts.flatMap(contextsOf);
    return [code, { misuse: contexts.filter((context) => OB_CONTEXTS[context] === "misuse"), legal: contexts.filter((context) => OB_CONTEXTS[context] === "legal") }];
  }));
}

