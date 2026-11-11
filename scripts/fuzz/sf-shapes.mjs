// The stored-function shapes (ADR 0964): the site is the body of a function the program stored (in an array, a map, an object's list) and the host's callback invokes
// through an unresolved call, after its `await`.
//
// The walk of a continuation follows the calls it can name: a project function, an accessor, a primitive, a standard-library declaration. A callback stored where nothing
// follows it and called through the name it was stored under (`callbacks.forEach((c) => c())`, `const fn = list[0]; fn()`, `registry.get(k)!()`, `bus.emit()` over the
// listeners the program pushed, `Promise.all(tasks.map((t) => t()))`) is a call to no function the walk knows, so a source read in the stored function warned
// UNTRACKED_READ_AFTER_AWAIT in Chrome while the program was certified clean (probes `handed-callback-R` and six more, batch 10b). Family D of the object-boundary axis
// (ob-shapes.mjs) is a construct the language runs without a call; this is a call the walk cannot resolve, so it is its own axis (`sf-`).
//
//   sf-<form>-<host>           misuse   the site is the body of a module-scope function, stored by the module and invoked by the host's callback after the compute's `await`
//   sf-<form>-<host>-never     legal    the same text, the stored function never invoked (the names are used and nothing calls)
//   sf-<form>-<host>-before    legal    the same text, invoked before the compute's `await`: the read is a dependency of the computation, Chrome reports nothing
//
//   forms: foreach   `list.forEach((c) => c())`        index   `list[0]()`               const   `const f = list[0]; f()`         mapget   `registry.get("k")!()`
//          bus       `bus.emit()` over the listeners   promise `Promise.all(tasks.map((t) => t()))`      param   `run(stored)` with `function run(f) { f(); }`
//   `param` is the control the walk already follows (a named function handed to a project helper that calls its parameter): a violation, as before.
//   hosts: `memo-async` and `effect-async`, SC1002's two async computes.
//
// Recall contexts only, one base shape (the direct binding): the question is how the stored function is invoked, not the spelling of the site.
import { awaitStmt, call, decl, domExpr, expr, fn, prim, ref, ret, num } from "./model.mjs";

/** Each form: `defs(n, h)` the module-scope text that stores the function `h`, and the host-side statements `run` (invokes it) and `idle` (names it, calls nothing). */
const FORMS = {
  foreach: (n, h) => { const l = n("cbs"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [expr(domExpr(`${l}.forEach((c) => c())`))], idle: [expr(domExpr(`void ${l}`))] }; },
  index: (n, h) => { const l = n("cbs"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [expr(domExpr(`${l}[0]()`))], idle: [expr(domExpr(`void ${l}`))] }; },
  const: (n, h) => { const l = n("cbs"); const f = n("fn"); return { defs: [`const ${l}: Array<() => unknown> = []`, `${l}.push(${h})`], run: [expr(domExpr(`const ${f} = ${l}[0]`)), expr(domExpr(`${f}()`))], idle: [expr(domExpr(`void ${l}`))] }; },
  mapget: (n, h) => { const m = n("reg"); return { defs: [`const ${m} = new Map<string, () => unknown>()`, `${m}.set("k", ${h})`], run: [expr(domExpr(`${m}.get("k")!()`))], idle: [expr(domExpr(`void ${m}`))] }; },
  bus: (n, h) => { const b = n("bus"); return { defs: [`const ${b} = { listeners: [] as Array<() => unknown>, emit() { this.listeners.forEach((l) => l()); } }`, `${b}.listeners.push(${h})`], run: [expr(domExpr(`${b}.emit()`))], idle: [expr(domExpr(`void ${b}`))] }; },
  promise: (n, h) => { const t = n("tasks"); return { defs: [`const ${t}: Array<() => Promise<unknown>> = []`, `${t}.push(${h})`], run: [expr(domExpr(`await Promise.all(${t}.map((t) => t()))`))], idle: [expr(domExpr(`void ${t}`))], async: true }; },
  param: (n, h) => { const r = n("run"); return { defs: [`function ${r}(f: () => unknown) { f(); }`], run: [expr(domExpr(`${r}(${h})`))], idle: [expr(domExpr(`void ${r}`))] }; },
};
export const SF_FORM_NAMES = Object.keys(FORMS);
export const SF_HOSTS = ["memo-async", "effect-async"];
/** `{ [context]: "misuse" | "legal" }`. */
export const SF_CONTEXTS = Object.fromEntries(SF_FORM_NAMES.flatMap((form) => SF_HOSTS.flatMap((host) => [[`sf-${form}-${host}`, "misuse"], [`sf-${form}-${host}-never`, "legal"], [`sf-${form}-${host}-before`, "legal"]])));
export const SF_CONTEXT_NAMES = Object.keys(SF_CONTEXTS);
export const SF_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

const NAME = new RegExp(`^sf-(${[...SF_FORM_NAMES].sort((a, b) => b.length - a.length).join("|")})-(${SF_HOSTS.join("|")})(-never|-before)?$`);
/** `{ form, host, never, before }` of an `sf-*` context name, or null. */
export const sfParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: match[3] === "-never", before: match[3] === "-before" } : null;
};

/** The base shapes an `sf-*` cell takes: the direct binding, no layer. */
export function sfShape(context, shape) {
  return !sfParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The host's async compute with `before` ahead of its `await` and `after` behind it (misuse.mjs `memo-async` and `effect-async`, with the statements split around the await). */
function hostOf(host, before, after, kit, asynchronous) {
  void asynchronous;
  const compute = fn([...before, awaitStmt(), ...after, ret(num(1))], { async: true });
  if (host === "memo-async") {
    const m = kit.name("m");
    return { body: [decl(m, prim("memo", compute)), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [] };
  }
  return { body: [expr(prim("effect", compute, { apply: fn([]) }))], view: [] };
}

/** The placement of `stmts` for an `sf-*` context (`{ body, view, hoisted }`), or undefined for any other. */
export function sfContext(context, stmts, kit) {
  const parts = sfParts(context);
  if (!parts) return undefined;
  const h = kit.name("hx");
  const form = FORMS[parts.form](kit.name, h);
  const stored = decl(h, fn(stmts, { form: "function", async: Boolean(form.async) }));
  const hoisted = [stored, ...form.defs.map((text) => expr(domExpr(text)))];
  const trigger = parts.never ? form.idle : form.run;
  const placed = parts.before ? hostOf(parts.host, trigger, [], kit) : hostOf(parts.host, [], trigger, kit);
  return { ...placed, hoisted };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function sfPlacement() {
  const names = SF_CONTEXT_NAMES;
  return Object.fromEntries(SF_CODES.map((code) => [code, { misuse: names.filter((name) => SF_CONTEXTS[name] === "misuse"), legal: names.filter((name) => SF_CONTEXTS[name] === "legal") }]));
}
