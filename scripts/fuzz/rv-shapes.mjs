// The spellings batch 10b's Fable review found silent in 10b's own target shapes (ADR 0963's member routes, ADR 0964's stored-function calls): twenty Chrome false cleans on main
// ec68d519d, type-clean against the real typings, that no cell generated because `ob-shapes.mjs` writes every member as a method and `sf-shapes.mjs` has no member slot, `.call`,
// reassigned `let`, tag or `new` form. Two axes in one file, each with the twins that keep the names used and run nothing:
//
//   rs-<form>-<host>           misuse   the site is the body of a module-scope function the program stored, invoked by the host's callback after the compute's `await` through a form
//   rs-<form>-<host>-never     legal    the same text, nothing invokes it
//   rs-<form>-<host>-before    legal    the same text, invoked before the compute's `await` (the read is a dependency of the computation)
//     forms: prop `bus.cb()` (a member the program assigned)   optprop `holder.cb?.()`   call `fn.call(undefined)`   apply `cbs[0].apply(undefined, [])`   let `fn()` over a reassigned `let`
//            tag `t\`x\`` (a stored tag)   tagidx `tags[0]\`x\``   new `new K()` over a stored function   propcall `bus.cb.call(undefined)`   iface `svc.run()` over an interface's
//            method signature   bind `fn.bind(undefined)()`   field `holder.cb()` over a class field   propmeth, thismeth: the slot called inside a method the callback calls
//            (`emitter.emit()` over `bus.cb()`, over `this.cb()`)   hosts: `memo-async`, `effect-async` (SC1002)
//
//   rm-<form>-<host>           misuse   the site is the body of a function a host-called member holds, written as a function VALUE (an arrow, a `function`, a named reference, an assignment,
//                                       a `defineProperty` descriptor), and the host's callback triggers the route
//   rm-<form>-<host>-never     legal    the same text, the route never triggered
//   rh-<form>-<host>[-never]   as rm-*, the route written at module scope (the review's own spelling: an object the host's callback only reads), not inside the host's callback
//     forms: tostring-arrow, valueof-fn, toprimitive-arrow, tojson-arrow (a coercion), proxy-get-arrow, proxy-set-arrow, proxy-has-arrow (a trap), iter-fn (a generator function as
//            `[Symbol.iterator]`), then-arrow (a thenable; the async hosts), fnname (`{ toString: show }`), assigned (`o.toString = () => …`), proto (`K.prototype.toString = function …`),
//            defprop-get, defprop-set (`Object.defineProperty(o, "x", { get: () => … })`)
//     hosts: `memo-async`, `effect-async` (SC1002: the route is triggered after the compute's `await`), `settled`, `tracked` (SC3001's leaf owners)
//
// Recall contexts only, one base shape (the direct binding): the question is how the function is reached, not the spelling of the site.
import { awaitStmt, call, contain, decl, domExpr, expr, fn, prim, ref, ret, num } from "./model.mjs";

const MAP = "{} as Record<string, number>";

/** B1: `defs(n, h)` the module-scope text that stores the function `h`, `run` the host-side statements that invoke it, `idle` the statements that only keep its names used. */
const CALL_FORMS = {
  prop: (n, h) => { const b = n("bus"); return { defs: [`const ${b}: { cb: () => unknown } = { cb: () => {} }`, `${b}.cb = ${h}`], run: [`${b}.cb()`], idle: [`void ${b}`] }; },
  optprop: (n, h) => { const b = n("hold"); return { defs: [`const ${b}: { cb?: () => unknown } = {}`, `${b}.cb = ${h}`], run: [`${b}.cb?.()`], idle: [`void ${b}`] }; },
  call: (n, h) => { const l = n("cbs"); const f = n("fn"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [`const ${f} = ${l}[0]`, `${f}.call(undefined)`], idle: [`void ${l}`] }; },
  apply: (n, h) => { const l = n("cbs"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [`${l}[0].apply(undefined, [])`], idle: [`void ${l}`] }; },
  let: (n, h) => { const f = n("fn"); return { defs: [`let ${f}: () => unknown = () => {}`, `${f} = ${h}`], run: [`${f}()`], idle: [`void ${f}`] }; },
  tag: (n, h) => { const l = n("tags"); const t = n("tag"); return { defs: [`const ${l}: Array<(s: TemplateStringsArray) => unknown> = []`, `${l}.push(${h})`], run: [`const ${t} = ${l}[0]`, `${t}\`x\``], idle: [`void ${l}`] }; },
  tagidx: (n, h) => { const l = n("tags"); return { defs: [`const ${l}: Array<(s: TemplateStringsArray) => unknown> = []`, `${l}.push(${h})`], run: [`${l}[0]\`x\``], idle: [`void ${l}`] }; },
  new: (n, h) => { const l = n("ctors"); const k = n("K"); return { defs: [`const ${l}: Array<new () => object> = []`, `${l}.push(${h} as unknown as new () => object)`], run: [`const ${k} = ${l}[0]`, `new ${k}()`], idle: [`void ${l}`] }; },
  propcall: (n, h) => { const b = n("bus"); return { defs: [`const ${b}: { cb: () => unknown } = { cb: () => {} }`, `${b}.cb = ${h}`], run: [`${b}.cb.call(undefined)`], idle: [`void ${b}`] }; },
  iface: (n, h) => { const b = n("svc"); const t = n("Svc"); return { defs: [`interface ${t} { run(): unknown }`, `const ${b}: ${t} = { run: ${h} }`], run: [`${b}.run()`], idle: [`void ${b}`] }; },
  bind: (n, h) => { const l = n("cbs"); const f = n("fn"); const g = n("bound"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [`const ${f} = ${l}[0]`, `const ${g} = ${f}.bind(undefined)`, `${g}()`], idle: [`void ${l}`] }; },
  field: (n, h) => { const c = n("Holder"); const b = n("holder"); return { defs: [`class ${c} { cb: () => unknown = () => {}; }`, `const ${b} = new ${c}()`, `${b}.cb = ${h}`], run: [`${b}.cb()`], idle: [`void ${b}`] }; },
  // The slot is called inside a method the host's callback calls (a member route: no walk of the continuation reaches inside it).
  propmeth: (n, h) => { const b = n("bus"); const e = n("emitter"); return { defs: [`const ${b}: { cb: () => unknown } = { cb: () => {} }`, `${b}.cb = ${h}`, `const ${e} = { emit() { ${b}.cb(); } }`], run: [`${e}.emit()`], idle: [`void ${e}`] }; },
  thismeth: (n, h) => { const c = n("Emitter"); const e = n("emitter"); return { defs: [`class ${c} { cb: () => unknown = () => {}; emit() { this.cb(); } }`, `const ${e} = new ${c}()`, `${e}.cb = ${h}`], run: [`${e}.emit()`], idle: [`void ${e}`] }; },
};
export const RS_FORM_NAMES = Object.keys(CALL_FORMS);
export const RS_HOSTS = ["memo-async", "effect-async"];
/** `{ [context]: "misuse" | "legal" }`. */
export const RS_CONTEXTS = Object.fromEntries(RS_FORM_NAMES.flatMap((form) => RS_HOSTS.flatMap((host) => [[`rs-${form}-${host}`, "misuse"], [`rs-${form}-${host}-never`, "legal"], [`rs-${form}-${host}-before`, "legal"]])));
export const RS_CONTEXT_NAMES = Object.keys(RS_CONTEXTS);
export const RS_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

/** B2: `(n) => { head, close, after, run, idle, nest? }`, the frame around the site (model.mjs `frame`, printed as raw text around the body of a function of its own). */
const object = (member, after, run) => (n) => { const o = n("o"); return { head: `const ${o} = { ${member} `, close: " };", after, run: run(o), idle: [`void ${o};`] }; };
const iterable = (member, after, consume) => (n) => { const o = n("o"); return { head: `const ${o}: Iterable<number> = { ${member} `, close: " };", after, run: consume(o), idle: [`void ${o};`] }; };
const proxy = (trap, after, run) => (n) => { const p = n("px"); return { head: `const ${p} = new Proxy(${MAP}, { ${trap} `, close: " });", after, run: run(p), idle: [`void ${p};`] }; };
const TEMPLATE = (o) => ["void `${" + o + "}`;"];
const PLUS = (o) => [`void (+${o});`];
export const RM_FORMS = {
  "tostring-arrow": object("toString: () =>", ['return "x";'], TEMPLATE),
  "valueof-fn": object("valueOf: function ()", ["return 1;"], PLUS),
  "toprimitive-arrow": object("[Symbol.toPrimitive]: () =>", ["return 1;"], PLUS),
  "tojson-arrow": object("toJSON: () =>", ["return 1;"], (o) => [`void JSON.stringify(${o});`]),
  "proxy-get-arrow": proxy("get: (_t, _k) =>", ["return 1;"], (p) => [`void ${p}.x;`]),
  "proxy-set-arrow": proxy("set: (_t, _k, _v) =>", ["return true;"], (p) => [`${p}.x = 1;`]),
  "proxy-has-arrow": proxy("has: (_t, _k) =>", ["return true;"], (p) => [`void ("x" in ${p});`]),
  "iter-fn": iterable("[Symbol.iterator]: function* ()", ["yield 1;"], (o) => [`void [...${o}];`]),
  "then-arrow": (n) => { const o = n("th"); return { head: `const ${o} = { then: (res: (v: number) => void) => `, close: " };", after: ["res(1);"], run: [`await ${o};`], idle: [`void ${o};`] }; },
  fnname: (n) => { const f = n("show"); const o = n("o"); return { head: `const ${f} = () => `, close: `;\nconst ${o} = { toString: ${f} };`, after: ['return "x";'], run: TEMPLATE(o), idle: [`void ${o};`] }; },
  assigned: (n) => { const o = n("o"); return { head: `const ${o}: { toString(): string } = {};\n${o}.toString = () => `, close: ";", after: ['return "x";'], run: TEMPLATE(o), idle: [`void ${o};`] }; },
  proto: (n) => { const k = n("K"); return { head: `class ${k} {}\n${k}.prototype.toString = function () `, close: ";", after: ['return "x";'], run: ["void `${new " + k + "()}`;"], idle: [`void ${k};`] }; },
  "defprop-get": (n) => { const o = n("o"); return { head: `const ${o} = {} as { x: number };\nObject.defineProperty(${o}, "x", { get: () => `, close: " });", after: ["return 1;"], run: [`void ${o}.x;`], idle: [`void ${o};`] }; },
  "defprop-set": (n) => { const o = n("o"); return { head: `const ${o} = {} as { x: number };\nObject.defineProperty(${o}, "x", { set: (_v: number) => `, close: " });", after: [], run: [`${o}.x = 1;`], idle: [`void ${o};`] }; },
};
export const RM_FORM_NAMES = Object.keys(RM_FORMS);
/** The host each code is placed in: SC1002's two async computes, SC3001's two leaf owners. `then-arrow` awaits, so only the async hosts take it. */
export const RM_HOSTS = ["memo-async", "effect-async", "settled", "tracked"];
const ASYNC_HOSTS = new Set(["memo-async", "effect-async"]);
export const RM_CODES = {
  UNTRACKED_READ_AFTER_AWAIT: ["memo-async", "effect-async"],
  CLEANUP_IN_FORBIDDEN_SCOPE: ["settled", "tracked"],
  PRIMITIVE_IN_FORBIDDEN_SCOPE: ["settled", "tracked"],
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: ["settled", "tracked"],
};
const formsOf = (host) => RM_FORM_NAMES.filter((form) => form !== "then-arrow" || ASYNC_HOSTS.has(host));
const rmContextsOf = (host) => formsOf(host).flatMap((form) => ["rm", "rh"].flatMap((prefix) => [`${prefix}-${form}-${host}`, `${prefix}-${form}-${host}-never`]));
export const RM_CONTEXTS = Object.fromEntries(RM_HOSTS.flatMap((host) => rmContextsOf(host).map((context) => [context, context.endsWith("-never") ? "legal" : "misuse"])));
export const RM_CONTEXT_NAMES = Object.keys(RM_CONTEXTS);

const RS_NAME = new RegExp(`^rs-(${[...RS_FORM_NAMES].sort((a, b) => b.length - a.length).join("|")})-(${RS_HOSTS.join("|")})(-never|-before)?$`);
/** `{ form, host, never, before }` of an `rs-*` context name, or null. */
export const rsParts = (context) => {
  const match = RS_NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: match[3] === "-never", before: match[3] === "-before" } : null;
};
const RM_NAME = new RegExp(`^r([mh])-(${[...RM_FORM_NAMES].sort((a, b) => b.length - a.length).join("|")})-(${RM_HOSTS.join("|")})(-never)?$`);
/** `{ form, host, never, hoisted }` of an `rm-*` (in the host's callback) or `rh-*` (at module scope) context name, or null. */
export const rmParts = (context) => {
  const match = RM_NAME.exec(context);
  return match && formsOf(match[3]).includes(match[2]) ? { form: match[2], host: match[3], never: Boolean(match[4]), hoisted: match[1] === "h" } : null;
};

/** The base shapes an `rs-*` or `rm-*` cell takes: the direct binding, no layer. */
export function rvShape(context, shape) {
  return (!rsParts(context) && !rmParts(context)) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The host's async compute with `before` ahead of its `await` and `after` behind it (sf-shapes.mjs `hostOf`). */
function hostOf(host, before, after, kit) {
  const compute = fn([...before, awaitStmt(), ...after, ret(num(1))], { async: true });
  if (host === "memo-async") {
    const m = kit.name("m");
    return { body: [decl(m, prim("memo", compute)), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
  }
  return { body: [expr(prim("effect", compute, { apply: fn([]) }))], view: [] };
}

/** The placement of `stmts` for an `rs-*` context (`{ body, view, hoisted }`), or undefined for any other. */
export function rsContext(context, stmts, kit) {
  const parts = rsParts(context);
  if (!parts) return undefined;
  const h = kit.name("hx");
  const form = CALL_FORMS[parts.form](kit.name, h);
  const hoisted = [decl(h, fn(stmts, { form: "function" })), ...form.defs.map((text) => expr(domExpr(text)))];
  const trigger = (parts.never ? form.idle : form.run).map((text) => expr(domExpr(text)));
  const placed = parts.before ? hostOf(parts.host, trigger, [], kit) : hostOf(parts.host, [], trigger, kit);
  return { ...placed, hoisted };
}

/**
 * The placement of `stmts` for an `rm-*` or `rh-*` context: the frame of the form around them. `rm-*`: the frame is written in the host's callback, which triggers it. `rh-*`: the frame is
 * written at module scope (`hoisted`) and the host's callback only triggers it (`place(host, inner)` is the plain host's own placement).
 */
export function rmContext(context, stmts, kit, place) {
  const parts = rmParts(context);
  if (!parts) return undefined;
  const { run, idle, ...frame } = RM_FORMS[parts.form](kit.name);
  if (!parts.hoisted) return place(parts.host, [contain("frame", 9, stmts, { ...frame, then: parts.never ? idle : run })]);
  const trigger = (parts.never ? idle : run).map((text) => expr(domExpr(text.replace(/;$/, ""))));
  return { ...place(parts.host, trigger), hoisted: [contain("frame", 9, stmts, frame)] };
}

/** `{ [code]: { misuse, legal } }` for SPEC: the `rs-*` cells of SC1002, the `rm-*` cells of each code's hosts. */
export function rvPlacement() {
  const out = {};
  for (const code of RS_CODES) out[code] = { misuse: RS_CONTEXT_NAMES.filter((name) => RS_CONTEXTS[name] === "misuse"), legal: RS_CONTEXT_NAMES.filter((name) => RS_CONTEXTS[name] === "legal") };
  for (const [code, hosts] of Object.entries(RM_CODES)) {
    const contexts = hosts.flatMap(rmContextsOf);
    out[code] = { misuse: [...(out[code]?.misuse ?? []), ...contexts.filter((name) => RM_CONTEXTS[name] === "misuse")], legal: [...(out[code]?.legal ?? []), ...contexts.filter((name) => RM_CONTEXTS[name] === "legal")] };
  }
  return out;
}
export const RV_CONTEXT_NAMES = [...RS_CONTEXT_NAMES, ...RM_CONTEXT_NAMES];
