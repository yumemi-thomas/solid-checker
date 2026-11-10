// The table-entry shapes (ADR 0935): one family of recall contexts for every row of the checker's own tables that decides what becomes
// of a function handed to a standard-library API (rust/crates/solid-reactive-ir/src/runtime_semantics.rs `timing_behavior`,
// `FRESH_STACK_SCHEDULERS`; carrier_runs.rs; local_access.rs `hand_off_at`).
//
// The checker keeps these rows by the compiler-selected qualified name, never by spelling (invariant 3). A row is a premise about the API:
// "`Set.add` only keeps the function", "`setTimeout` runs it from a task", "`Array.map` runs it before returning". Each row gets a pair that
// differs only in the fact the premise claims, plus the exactness twin:
//
//   real    the exact API. Inline rows run the function where the call sits (the body: a misuse; a handler: legal). Host-run rows
//           (schedulers, observers, listeners) run it later, with no project run: legal; the rows whose host can be made to call back on the
//           caller's stack (a listener and `dispatchEvent`, a thenable, a bound function, a collection the body iterates) have a `sync`/`run`
//           variant where the project runs it inside a window: a misuse.
//   shadow  a declaration of the same spelling with the opposite behaviour (invariant 3: a name-only match is wrong either way). A local
//           `setTimeout` that calls its callback must not be a fresh-stack scheduler; a local `map` that only stores must not run the function.
//
// A context name is `tbl-<row slug>-<variant>`; the variant names where the function is handed (`b` body, `u` closed `untrack`, an
// allowed region: ADR 0927's rows lived there) and where it runs (`h` handler). `placement(SPEC)` puts each context on a code's misuse or
// legal side by the base context its run is in (SPEC[code].contexts / legal), as ADR 0927 and 0932 did for `kept-*` and `evt-row-*`.
// Base shapes only (misuse.mjs BASE_ONLY_CONTEXTS), the plain shape of each variant (misuse.mjs `fireShape`).
import { button, call, decl, domExpr, expr, fn, num, prim, ref, ret } from "./model.mjs";

/** The codes the family is placed for: a read, a write and an action at the site (the three the hand-off roles decide). */
export const TABLE_CODES = ["STRICT_READ_UNTRACKED", "REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"];

const slugOf = (name) => name.toLowerCase().replaceAll(".", "-");

/**
 * The rows. A row is `{ name, kind, ... }` with `name` the qualified name the checker's table keys on (the compiler's, `lib.d.ts`'s), and:
 *   recv, m, args, fn, ctor   the call: `recv.m(args)`, `fn(args)` or `new ctor(args)`; `$f` in `args` is the function, `$r` the receiver variable
 *   recvDecl                  the initializer of `$r` when the row needs the receiver in a variable (a collection the body iterates)
 *   ret                       what the function returns when the callback type asks for a value (`0`, `""`)
 *   zero                      a receiver on which the callback never runs (an inline row), when there is one
 *   sync / run                a host row's synchronous path / a kept row's project run (text, `$r` the receiver)
 *   after                     statements that make a host run the callback at all (`$r.observe(...)`)
 */
const rows = [];
const row = (name, kind, spec) => rows.push({ name, kind, slug: slugOf(name), ...spec });

// ---- InlineCallback rows: run before the call returns, on the caller's stack (hand_off_at: HandOff::Inline) -------------------------------
const TYPED_AWAY_ES2023 = "typed away: the fuzz tsconfig's lib is ES2022, so `findLast`/`findLastIndex` are TS2550/TS2339 and tsc reports the call (invariant 1)";
const ARRAYISH = [
  ["forEach", [], {}], ["map", [], {}], ["flatMap", [], {}], ["filter", [], {}], ["some", [], {}], ["every", [], {}], ["find", [], {}], ["findIndex", [], {}],
  ["findLast", [], { noRow: TYPED_AWAY_ES2023 }], ["findLastIndex", [], { noRow: TYPED_AWAY_ES2023 }], ["reduce", ["0"], { ret: "0" }], ["reduceRight", ["0"], { ret: "0" }],
];
for (const [kind, receiver] of [["Array", "[1]"], ["ReadonlyArray", "([1] as readonly number[])"]]) {
  for (const [m, extra, spec] of ARRAYISH) {
    // `reduce` with one element and no initial value returns it uncalled; the others skip an empty receiver.
    const zero = m.startsWith("reduce") ? `${receiver}` : kind === "Array" ? "([] as number[])" : "([] as readonly number[])";
    row(`${kind}.${m}`, "inline", { recv: receiver, m, args: ["$f", ...extra], zero, zeroArgs: m.startsWith("reduce") ? ["$f"] : undefined, ...spec });
  }
}
row("Array.sort", "inline", { recv: "[2, 1]", m: "sort", args: ["$f"], ret: "0", zero: "([] as number[])" });
row("Set.forEach", "inline", { recv: "new Set([1])", m: "forEach", args: ["$f"], zero: "new Set<number>()" });
row("Map.forEach", "inline", { recv: "new Map([[1, 1]])", m: "forEach", args: ["$f"], zero: "new Map<number, number>()" });
row("NodeList.forEach", "inline", { recv: "(document.body.childNodes as NodeList)", m: "forEach", args: ["$f"], zero: '(document.createElement("i").childNodes as NodeList)' });
row("NodeListOf.forEach", "inline", { recv: 'document.querySelectorAll("div")', m: "forEach", args: ["$f"], zero: 'document.querySelectorAll("sc-no-such-tag")' });
row("String.replace", "inline", { recv: '"x"', m: "replace", args: ['"x"', "$f"], ret: '"y"', zero: '"x"', zeroArgs: ['"z"', "$f"] });
row("String.replaceAll", "inline", { recv: '"x"', m: "replaceAll", args: ['"x"', "$f"], ret: '"y"', zero: '"x"', zeroArgs: ['"z"', "$f"] });
// `f.call(null)` / `f.apply(null, [])`: the receiver is the function, which `Function.prototype.call`/`apply` invoke before they return (carrier_runs.rs:86).
// No shadow: the function's own `call` is the API; a program that replaced it would be a different premise (a patched prototype: the `patch-*` contexts).
row("CallableFunction.call", "inline", { recv: "$f", m: "call", args: ["null"], suffix: "-receiver", noShadow: true });
row("CallableFunction.apply", "inline", { recv: "$f", m: "apply", args: ["null", "[]"], noShadow: true });
row("Function.call", "inline", { recv: "($f as Function)", m: "call", args: ["null"], noShadow: true });
row("Function.apply", "inline", { recv: "($f as Function)", m: "apply", args: ["null", "[]"], noShadow: true });
row("NewableFunction.call", "inline", { recv: "$f", m: "call", args: ["null"], noRow: "`call` on a constructor type throws at run time (a class constructor invoked without `new`), so no program runs the row" });
row("NewableFunction.apply", "inline", { recv: "$f", m: "apply", args: ["null", "[]"], noRow: "`apply` on a constructor type throws at run time (a class constructor invoked without `new`), so no program runs the row" });
row("JSON.parse", "inline", { recv: "JSON", m: "parse", args: ['"1"', "$f"], ret: "0", global: true });
row("JSON.stringify", "inline", { recv: "JSON", m: "stringify", args: ["1", "$f"], ret: "0", global: true });
row("Reflect.apply", "inline", { recv: "Reflect", m: "apply", args: ["$f", "null", "[]"], global: true });
row("PromiseConstructor.construct", "inline", { ctor: "Promise", args: ["$f"], global: true });
row("ArrayConstructor.from", "inline", { recv: "Array", m: "from", args: ["[1]", "$f"], ret: "0", zeroArgs: ["[]", "$f"], global: true });
for (const typed of ["Int8", "Uint8", "Uint8Clamped", "Int16", "Uint16", "Int32", "Uint32", "Float32", "Float64"]) {
  row(`${typed}ArrayConstructor.from`, "inline", { recv: `${typed}Array`, m: "from", args: ["[1]", "$f"], ret: "0", zeroArgs: ["[]", "$f"], global: true });
}
for (const typed of ["BigInt64", "BigUint64"]) {
  row(`${typed}ArrayConstructor.from`, "inline", { recv: `${typed}Array`, m: "from", args: ["[1n]", "$f"], ret: "0n", zeroArgs: ["[]", "$f"], global: true });
}

// ---- FreshStackCallback rows (FRESH_STACK_SCHEDULERS): a task or a microtask runs it, after the call returns, on an empty stack ---------
row("queueMicrotask", "fresh", { fn: "queueMicrotask", args: ["$f"] });
row("WindowOrWorkerGlobalScope.queueMicrotask", "fresh", { recv: "window", m: "queueMicrotask", args: ["$f"] });
row("setTimeout", "fresh", { fn: "setTimeout", args: ["$f", "0"] });
row("setInterval", "fresh", { fn: "setInterval", args: ["$f", "20"], after: ["setTimeout(() => clearInterval($r), 70)"], keep: "$r" });
// `cancel`: the rendering opportunities these wait for come in the real-Chrome oracle and not in the fast host's virtual clock (the oracle sample's
// `oracle differs ... chrome-only` rows, 2026-11-10), so the host-only twin cancels the request and runs in neither: a program both hosts see the same.
row("requestAnimationFrame", "fresh", { fn: "requestAnimationFrame", args: ["$f"], keep: "$r", cancel: ["cancelAnimationFrame($r)"] });
row("requestIdleCallback", "fresh", { fn: "requestIdleCallback", args: ["$f"], keep: "$r", cancel: ["cancelIdleCallback($r)"] });
row("WindowOrWorkerGlobalScope.setTimeout", "fresh", { recv: "window", m: "setTimeout", args: ["$f", "0"] });
row("WindowOrWorkerGlobalScope.setInterval", "fresh", { recv: "window", m: "setInterval", args: ["$f", "20"], after: ["setTimeout(() => window.clearInterval($r), 70)"], keep: "$r" });
row("AnimationFrameProvider.requestAnimationFrame", "fresh", { recv: "window", m: "requestAnimationFrame", args: ["$f"], keep: "$r", cancel: ["window.cancelAnimationFrame($r)"] });
row("Window.requestIdleCallback", "fresh", { recv: "window", m: "requestIdleCallback", args: ["$f"], keep: "$r", cancel: ["window.cancelIdleCallback($r)"] });
row("Promise.then", "fresh", { recv: "Promise.resolve(1)", m: "then", args: ["$f"] });
row("Promise.catch", "fresh", { recv: "Promise.reject(1)", m: "catch", args: ["$f"] });
row("Promise.finally", "fresh", { recv: "Promise.resolve(1)", m: "finally", args: ["$f"] });
row("Scheduler.postTask", "fresh", { recv: "scheduler", m: "postTask", args: ["$f"], noRow: "typed away in the real-Chrome oracle: its TypeScript has no `Scheduler` in lib.dom (type-error), while tsrs and the fast host accept it, so a sampled program is a harness difference" });
row("MediaSession.setActionHandler", "fresh", { recv: "navigator.mediaSession", m: "setActionHandler", args: ['"play"', "$f"], fnIndex: 1 });
row("IntersectionObserver.construct", "fresh", { ctor: "IntersectionObserver", args: ["$f"], keep: "$r", after: ["$r.observe(document.body)"], cancel: ["$r.observe(document.body)", "$r.disconnect()"] });
row("ResizeObserver.construct", "fresh", { ctor: "ResizeObserver", args: ["$f"], keep: "$r", after: ["$r.observe(document.body)"], cancel: ["$r.observe(document.body)", "$r.disconnect()"] });
row("MutationObserver.construct", "fresh", { ctor: "MutationObserver", args: ["$f"], keep: "$r", after: ["$r.observe(document.body, { childList: true })", 'document.body.append(document.createElement("i"))'] });
row("PerformanceObserver.construct", "fresh", { ctor: "PerformanceObserver", args: ["$f"], keep: "$r", after: ['$r.observe({ entryTypes: ["mark"] })', 'performance.mark("sc-mark")'], cancel: ['$r.observe({ entryTypes: ["mark"] })', "$r.disconnect()"] });
row("ReportingObserver.construct", "fresh", { ctor: "ReportingObserver", args: ["$f"], keep: "$r", after: ["$r.observe()"], cancel: ["$r.observe()", "$r.disconnect()"] });

// ---- DeferredCallback rows that are not fresh-stack: run by a host a synchronous path can reach (runs_on_invoker_stack) ----------------
row("addEventListener", "host", { fn: "addEventListener", args: ['"sc-evt"', "$f"], fnIndex: 1, sync: 'dispatchEvent(new Event("sc-evt"))',
  noRow: "withheld from this slice: the bare-global listener's synchronous-dispatch context is a false clean on main (fix 7); it returns with the wrapper family, after fix 7" });
row("Window.addEventListener", "host", { recv: "window", m: "addEventListener", args: ['"sc-evt"', "$f"], fnIndex: 1, sync: 'window.dispatchEvent(new Event("sc-evt"))' });
row("EventTarget.addEventListener", "host", { recv: "$r", m: "addEventListener", args: ['"sc-evt"', "$f"], fnIndex: 1, recvDecl: "new EventTarget()", sync: '$r.dispatchEvent(new Event("sc-evt"))' });
row("MediaQueryList.addListener", "host", { recv: "$r", m: "addListener", args: ["$f"], recvDecl: 'window.matchMedia("all")', sync: '$r.dispatchEvent(new Event("change"))' });
row("PromiseLike.then", "thenable", { recv: "$r", m: "then", args: ["$f"] });
row("CallableFunction.bind", "bind", { recv: "$r", m: "bind", args: ["null", "$f"], fnIndex: 1 });
row("Function.bind", "bind", { recv: "$r", m: "bind", args: ["null", "$f"], fnIndex: 1, fnType: "Function" });
row("NewableFunction.bind", "bind", { recv: "$r", m: "bind", args: ["null", "$f"], fnIndex: 1, noRow: "`bind` on a constructor type returns a constructor the project would have to `new`; the callback is a constructor argument, not a call: no program runs the row" });
row("BigIntConstructor.call", "value", { fn: "BigInt", args: ["$f as unknown as number"], noRow: "`BigInt(f)` throws a SyntaxError at run time (a function is not a number), so no program runs the row" });
row("Geolocation.getCurrentPosition", "host", { recv: "navigator.geolocation", m: "getCurrentPosition", args: ["$f"], noSync: "the callback-with-error path needs a document that is not fully active (the fuzz page's is)" });
row("Geolocation.watchPosition", "host", { recv: "navigator.geolocation", m: "watchPosition", args: ["$f"], noSync: "the callback-with-error path needs a document that is not fully active (the fuzz page's is)" });

// ---- RetainedValue rows: a collection keeps the function for project code (hand_off_at: HandOff::Unfollowed, ADR 0927) -------------------
row("Array.push", "kept", { recv: "$r", m: "push", args: ["$f"], recvDecl: "[] as Array<() => void>", run: "$r.forEach((g) => g())" });
row("Array.unshift", "kept", { recv: "$r", m: "unshift", args: ["$f"], recvDecl: "[] as Array<() => void>", run: "$r.forEach((g) => g())" });
row("Set.add", "kept", { recv: "$r", m: "add", args: ["$f"], recvDecl: "new Set<() => void>()", run: "$r.forEach((g) => g())" });
row("WeakSet.add", "kept", { recv: "$r", m: "add", args: ["$f"], recvDecl: "new WeakSet<() => void>()", noRun: "a WeakSet cannot be iterated: the project cannot get the function back" });
row("Map.set", "kept", { recv: "$r", m: "set", args: ["1", "$f"], fnIndex: 1, recvDecl: "new Map<number, () => void>()", run: "$r.forEach((g) => g())" });
row("WeakMap.set", "kept", { recv: "$r", m: "set", args: ["sc_key", "$f"], fnIndex: 1, recvDecl: "new WeakMap<object, () => void>()", keyDecl: "{}", run: "$r.get(sc_key)?.()" });

// ---- ValueOnly rows: the API reads or copies the argument and never calls it (hand_off_at: HandOff::Value) -----------------------------
for (const [name, spec] of [
  ["Window.removeEventListener", { recv: "window", m: "removeEventListener", args: ['"sc-evt"', "$f"], fnIndex: 1 }],
  ["EventTarget.removeEventListener", { recv: "new EventTarget()", m: "removeEventListener", args: ['"sc-evt"', "$f"], fnIndex: 1 }],
  ["CallableFunction.call", { recv: "((_t: unknown) => 1)", m: "call", args: ["$f", "1"] }],
  ["CallableFunction.bind", { recv: "((_t: unknown) => 1)", m: "bind", args: ["$f"], suffix: "-this" }],
  ["StringConstructor.call", { fn: "String", args: ["$f"] }],
  ["NumberConstructor.call", { fn: "Number", args: ["$f"] }],
  ["BooleanConstructor.call", { fn: "Boolean", args: ["$f"] }],
  ["SymbolConstructor.call", { fn: "Symbol", args: ["$f as unknown as string"] }],
  ["ObjectConstructor.call", { fn: "Object", args: ["$f"] }],
  ["ObjectConstructor.entries", { recv: "Object", m: "entries", args: ["$f"], global: true }],
  ["ObjectConstructor.keys", { recv: "Object", m: "keys", args: ["$f"], global: true }],
  ["ObjectConstructor.values", { recv: "Object", m: "values", args: ["$f"], global: true }],
  ["ObjectConstructor.assign", { recv: "Object", m: "assign", args: ["{}", "$f"], fnIndex: 1, global: true }],
  ["ArrayConstructor.construct", { ctor: "Array", args: ["$f as unknown as number"] }],
  // `new Set(f)` throws (a function is not iterable), and `[f]` is a part of a value (HandOff::Unfollowed), not this row: no program runs it.
  ["SetConstructor.construct", { ctor: "Set", args: ["[$f]"], noRow: "a function is not iterable (`new Set(f)` throws); `[f]` is a part of a value, HandOff::Unfollowed" }],
  ["MapConstructor.construct", { ctor: "Map", args: ["[[1, $f]]"], noRow: "a function is not iterable (`new Map(f)` throws); `[[1, f]]` is a part of a value, HandOff::Unfollowed" }],
  ["WeakSetConstructor.construct", { ctor: "WeakSet", args: ["[$f]"], noRow: "a function is not iterable (`new WeakSet(f)` throws); `[f]` is a part of a value, HandOff::Unfollowed" }],
  ["WeakMapConstructor.construct", { ctor: "WeakMap", args: ["[[$f, 1]]"], noRow: "a function is not iterable (`new WeakMap(f)` throws); `[[f, 1]]` is a part of a value, HandOff::Unfollowed" }],
  ["ArrayConstructor.isArray", { recv: "Array", m: "isArray", args: ["$f"], global: true }],
  ["NumberConstructor.isFinite", { recv: "Number", m: "isFinite", args: ["$f"], global: true }],
  ["NumberConstructor.isInteger", { recv: "Number", m: "isInteger", args: ["$f"], global: true }],
  ["NumberConstructor.isNaN", { recv: "Number", m: "isNaN", args: ["$f"], global: true }],
  ["NumberConstructor.isSafeInteger", { recv: "Number", m: "isSafeInteger", args: ["$f"], global: true }],
  ["parseFloat", { fn: "parseFloat", args: ["$f as unknown as string"] }],
  ["parseInt", { fn: "parseInt", args: ["$f as unknown as string"] }],
  ["Reflect.set", { recv: "Reflect", m: "set", args: ["$r", '"k"', "$f"], fnIndex: 2, global: true, recvDecl: "{} as Record<string, () => void>", run: "$r.k()" }],
  ["Reflect.get", { recv: "Reflect", m: "get", args: ["$f", '"k"'], global: true }],
  ["Reflect.has", { recv: "Reflect", m: "has", args: ["$f", '"k"'], global: true }],
  ["Reflect.deleteProperty", { recv: "Reflect", m: "deleteProperty", args: ["$f", '"k"'], global: true }],
]) row(name, "value", { ...spec });

for (const candidate of rows) if (candidate.suffix) candidate.slug += candidate.suffix;
/** Every row that has a program (a row with `noRow` is a stated gap: see `TABLE_GAPS`). */
export const TABLE_ROWS = rows.filter((candidate) => !candidate.noRow);
/** The rows no program can express, with the reason. */
export const TABLE_GAPS = Object.fromEntries(rows.filter((candidate) => candidate.noRow).map((candidate) => [candidate.name, candidate.noRow]));
export const rowBySlug = (slug) => TABLE_ROWS.find((candidate) => candidate.slug === slug);

// ---- variants -------------------------------------------------------------------------------------------------------------------------
// `hand`: where the function is declared and handed over (b the body, u a closed `untrack`: an allowed region). `run`: where it runs, if
// anywhere (b, u, or h a click handler): the base context its side comes from. `impl`: real, or a shadow that runs the function (`run`) or
// does not (`norun`).
const SAME = (hand) => ({ hand, run: hand, impl: "real" });
const VARIANTS = {
  inline: {
    b: SAME("b"), u: SAME("u"), h: SAME("h"),
    zero: { hand: "b", run: null, impl: "real", zero: true },
    norun: { hand: "b", run: null, impl: "norun" },
  },
  fresh: {
    "host-b": { hand: "b", run: null, impl: "real" }, "host-u": { hand: "u", run: null, impl: "real" },
    "shadow-b": { hand: "b", run: "b", impl: "run" }, "shadow-u": { hand: "u", run: "u", impl: "run" },
  },
  host: {
    "host-b": { hand: "b", run: null, impl: "real" }, "host-u": { hand: "u", run: null, impl: "real" },
    "sync-b": { hand: "b", run: "b", impl: "real", sync: true }, "sync-u-b": { hand: "u", run: "b", impl: "real", sync: true },
    "sync-u-h": { hand: "u", run: "h", impl: "real", sync: true },
    "shadow-b": { hand: "b", run: "b", impl: "run" }, "shadow-u": { hand: "u", run: "u", impl: "run" },
  },
  thenable: {
    "host-b": { hand: "b", run: null, impl: "real" },
    b: { hand: "b", run: "b", impl: "real", sync: true }, u: { hand: "u", run: "u", impl: "real", sync: true }, h: { hand: "h", run: "h", impl: "real", sync: true },
    "shadow-b": { hand: "b", run: "b", impl: "run" },
  },
  bind: {
    "host-b": { hand: "b", run: null, impl: "real" }, "host-u": { hand: "u", run: null, impl: "real" },
    "sync-b": { hand: "b", run: "b", impl: "real", sync: true }, "sync-u-b": { hand: "u", run: "b", impl: "real", sync: true },
    "sync-u-h": { hand: "u", run: "h", impl: "real", sync: true },
  },
  kept: {
    "host-b": { hand: "b", run: null, impl: "real" }, "host-u": { hand: "u", run: null, impl: "real" },
    "run-b": { hand: "b", run: "b", impl: "real", sync: true }, "run-u-b": { hand: "u", run: "b", impl: "real", sync: true },
    "run-u-h": { hand: "u", run: "h", impl: "real", sync: true },
    "shadow-b": { hand: "b", run: "b", impl: "run" }, "shadow-u": { hand: "u", run: "u", impl: "run" },
  },
  value: {
    "host-b": { hand: "b", run: null, impl: "real" },
    "called-b": { hand: "b", run: "b", impl: "real", called: true },
    "shadow-b": { hand: "b", run: "b", impl: "run" },
  },
};
const baseOf = { b: "body", u: "untrack", h: "handler" };

/** Every context of the family: `{ context, row, kind, variant, hand, run, base, impl, always }`. `base` is where the function runs, null if it never does in a window. */
export function tableContexts() {
  const contexts = [];
  for (const candidate of TABLE_ROWS) {
    for (const [variant, spec] of Object.entries(VARIANTS[candidate.kind])) {
      if (spec.sync && (candidate.noSync || candidate.noRun)) continue;
      if (spec.impl !== "real" && candidate.noShadow) continue;
      if (spec.zero && !candidate.zero && !candidate.zeroArgs) continue;
      contexts.push({ context: `tbl-${candidate.slug}-${variant}`, row: candidate.name, kind: candidate.kind, variant, hand: spec.hand, run: spec.run, base: spec.run ? baseOf[spec.run] : null, impl: spec.impl });
    }
  }
  return contexts;
}
export const TABLE_CONTEXT_NAMES = tableContexts().map((entry) => entry.context);

// ---- the programs ---------------------------------------------------------------------------------------------------------------------
const subst = (text, vars) => text.replaceAll("$f", vars.f).replaceAll("$r", vars.r).replaceAll("sc_key", vars.key);

/** The call as text: `recv.m(args)`, `fn(args)`, `new ctor(args)`; `args` may be overridden (a zero-run receiver). */
function callText(candidate, vars, { args = candidate.args, recv = candidate.recv, name = null } = {}) {
  const list = args.map((arg) => subst(arg, vars)).join(", ");
  if (candidate.ctor) return `new ${name ?? candidate.ctor}(${list})`;
  if (candidate.fn) return `${name ?? candidate.fn}(${list})`;
  return `${subst(recv, vars)}.${candidate.m}(${list})`;
}

/**
 * The shadow of a row's API: a declaration of the same spelling in the program's own scope, and the call that reaches it. `runs` makes it call
 * the function in the argument position (`fnIndex`, default the first `$f`); otherwise it only keeps it. Parameters are `unknown[]`, so the
 * declaration type-checks against any argument list.
 */
function shadowOf(candidate, vars, runs) {
  const at = candidate.fnIndex ?? Math.max(0, candidate.args.findIndex((arg) => arg.includes("$f")));
  const body = runs ? `(a[${at}] as () => void)();` : "void a;";
  const member = `(...a: unknown[]) => { ${body} }`;
  if (candidate.ctor) return { decl: `class ${candidate.ctor} { constructor(...a: unknown[]) { ${body} } }`, call: callText(candidate, vars) };
  if (candidate.fn) return { decl: `const ${candidate.fn} = ${member};`, call: callText(candidate, vars) };
  const list = candidate.args.map((arg) => subst(arg, vars)).join(", ");
  // The receiver of a member call is the program's variable (`$r`, a collection), or a path from a global (`Array`, `JSON`, `window`,
  // `navigator.mediaSession`): the shadow is an object in its place, nested to the path.
  if (candidate.recv === "$r") return { decl: `const ${vars.r} = { ${candidate.m}: ${member} };`, call: `${vars.r}.${candidate.m}(${list})`, receiver: true };
  if (/^[A-Za-z_][A-Za-z0-9_]*(\.[A-Za-z_][A-Za-z0-9_]*)*$/.test(candidate.recv)) {
    const [root, ...path] = candidate.recv.split(".");
    const nested = path.reduceRight((inner, key) => `{ ${key}: ${inner} }`, `{ ${candidate.m}: ${member} }`);
    return { decl: `const ${root} = ${nested} as unknown as any;`, call: callText(candidate, vars) };
  }
  // An expression receiver (`[1]`, `new Set([1])`): the program's own object in its place.
  return { decl: `const ${vars.s} = { ${candidate.m}: ${member} };`, call: `${vars.s}.${candidate.m}(${list})` };
}

/**
 * The placement of `stmts` for a `tbl-*` context (`{ body, view }`), or undefined for any other context. The site is inside the function `f`
 * the API is handed; `hand` is where `f` is declared and handed over, `run` where the project (or a shadow) runs it.
 */
export function tableContext(context, stmts, kit) {
  if (!context.startsWith("tbl-")) return undefined;
  const entry = tableContexts().find((candidate) => candidate.context === context);
  if (!entry) throw new Error(`no table context ${context}`);
  const candidate = TABLE_ROWS.find((row_) => row_.name === entry.row && row_.kind === entry.kind);
  const spec = VARIANTS[entry.kind][entry.variant];
  const vars = { f: kit.name("tf"), r: kit.name("tr"), s: kit.name("ts"), key: kit.name("tk") };
  const typed = candidate.ret ? [...stmts, ret(domExpr(candidate.ret))] : stmts;
  const func = decl(vars.f, fn(typed));
  const node = (text) => expr(domExpr(subst(text, vars)));
  const setup = [];
  let handStatements;
  let runStatements = [];
  const keep = (text) => (candidate.keep ? `const ${vars.r} = ${text}` : text);
  const real = spec.impl === "real";
  if (candidate.keyDecl) setup.push(node(`const ${vars.key} = ${candidate.keyDecl}`));
  if (entry.kind === "thenable") {
    // A thenable the program owns, typed `PromiseLike` for the real row: its `then` calls the callback at once, or never.
    const calls = spec.impl === "run" || spec.sync;
    const object = `{ then: (cb?: (v: number) => unknown) => { ${calls ? "cb?.(1);" : "void cb;"} } }`;
    setup.push(node(spec.impl === "real" ? `const ${vars.r} = ${object} as unknown as PromiseLike<number>` : `const ${vars.r} = ${object}`));
    handStatements = [func, node(callText(candidate, vars))];
  } else if (entry.kind === "bind") {
    setup.push(node(`const ${vars.s}${candidate.fnType ? `: ${candidate.fnType}` : ""} = (cb: () => void) => { cb(); }`), node(`let ${vars.r}: (() => void) | undefined`));
    handStatements = [func, node(`${vars.r} = ${vars.s}.bind(null, ${vars.f})`)];
    if (spec.sync) runStatements = [node(`${vars.r}?.()`)];
  } else if (spec.impl !== "real") {
    const shadow = shadowOf(candidate, vars, spec.impl === "run");
    if (candidate.recvDecl && !shadow.receiver) setup.push(node(`const ${vars.r} = ${candidate.recvDecl}`));
    handStatements = [func, node(shadow.decl), node(keep(shadow.call))];
  } else {
    if (candidate.recvDecl) setup.push(node(`const ${vars.r} = ${candidate.recvDecl}`));
    const args = spec.zero && candidate.zeroArgs ? candidate.zeroArgs : candidate.args;
    const recv = spec.zero && candidate.zero ? candidate.zero : candidate.recv;
    handStatements = [func, node(keep(callText(candidate, vars, { args, recv })))];
    for (const text of candidate.cancel ?? candidate.after ?? []) handStatements.push(node(text));
    if (spec.sync && candidate.sync) runStatements = [node(candidate.sync)];
    if (spec.sync && candidate.run) runStatements = [node(candidate.run)];
    if (spec.called) runStatements = [expr(call(ref(vars.f)))];
  }
  // Where the hand-off and the run sit.
  const inUntrack = spec.hand === "u";
  const handInHandler = spec.hand === "h";
  const body = [...setup];
  const view = [];
  const handPlaced = inUntrack ? [expr(prim("untrack", fn(handStatements)))] : handStatements;
  if (handInHandler) {
    view.push(button(kit.name("b"), [...handStatements, ...runStatements]));
  } else {
    body.push(...handPlaced);
    if (runStatements.length) {
      if (spec.run === "h") view.push(button(kit.name("b"), runStatements));
      else body.push(...runStatements);
    }
  }
  return { body, view };
}

/**
 * `{ [code]: { misuse, legal } }` for `SPEC` (misuse.mjs merges it into each code's recall lists): a context whose function runs at `base`
 * is a misuse of the code where `base` is one of its misuse contexts and legal where it is one of its legal ones (a read in a handler or in an
 * `untrack` is legal though neither is listed so); a context whose function never runs in a window is legal for every code.
 */
export function tablePlacement(SPEC) {
  const placement = {};
  for (const code of TABLE_CODES) {
    const spec = SPEC[code];
    const read = code === "STRICT_READ_UNTRACKED";
    const misuse = [];
    const legal = [];
    for (const entry of tableContexts()) {
      if (entry.base === null) legal.push(entry.context);
      else if (spec.contexts.includes(entry.base)) misuse.push(entry.context);
      else if ((spec.legal ?? []).includes(entry.base) || (read && entry.base === "handler")) legal.push(entry.context);
    }
    placement[code] = { misuse, legal };
  }
  return placement;
}
