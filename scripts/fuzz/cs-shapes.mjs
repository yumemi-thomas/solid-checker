// The computation-start shapes (batch 11, slice 9B, ADR 0973): the site is a read written after the `await` of an async function the program starts as a tracked async computation.
// Chrome (rc.21) runs the function as the compute of `createMemo` or `createEffect` and reports UNTRACKED_READ_AFTER_AWAIT at the read, while a program with no computation the
// producers' index finds was clean on main 177402dfb (the oracle cases `after-await-universe-census-*`: no SC1002 row at the read; SC1001's `function-run-unplaced` row beside it
// claims another code). The accounting of slice 9B (`after_await_accounting.rs`) keeps SC1002's uncertifiable row there. The review of #233 found more silent paths of the same pass,
// and each is a route of this family (below).
//
//   cs-<form>-<host>[-<route>]          misuse   the function the form starts runs as the host's compute: the read after its `await` is untracked
//   cs-<form>-<host>[-<route>]-never    legal    the same text, nothing starts the function (the names stay used)
//   cs-<form>-<host>[-<route>]-before   legal    the same program with the read (the call that reaches it) ahead of the `await`: a dependency of the computation, Chrome reports nothing
//
//   forms (how the function is started):
//          wrapper  `function track(compute) { return createMemo(compute); }`, `track(work)`: the producers' hand-off (ADR 0873) follows a function literal and an arrow through such a
//                   wrapper, not a function declaration handed by identifier
//          call     `(createMemo as ...).call(undefined, work)`
//          reflect  `Reflect.apply(createMemo as ..., undefined, [work])`
//          element  `[createMemo as ...][0](work)`
//          rec      the compute written inline in the primitive, a start the index recognizes (`createMemo(async () => …)`): the routes that need a recognized computation
//   hosts: `memo` (`createMemo`) and `effect` (`createEffect`'s compute), SC1002's two async computes.
//
//   routes (how the continuation reaches the site):
//          (none)       the read is written in the started function
//          getter       in a getter of a module-level object, which a project function reads (`work` calls `peek()` after its `await`, `peek` returns `holder.value`): the callee reason
//                       the first build of slice 9B had rested on the member-route net, which runs only where a computation is recognized
//          ns-method, ns-arrow, ns-computed, ns-optional   B2 of the review of #233: a method, an arrow property, a computed member (`ns["run"]()`) and an optional member (`ns?.run()`)
//                       of a module-scope `const` object literal the receiver census withholds. The lookup names no project function: the call left the pass through a `continue`
//          dotcall, apply, reflect, iife, iife-fn, iife-async   B3: `f.call(…)`, `f.apply(…)`, `Reflect.apply(f, …)`, and a function literal called where it is written (an arrow, a
//                       function expression, an async arrow awaited): each runs a project function on the continuation's stack
//          gen, proxy-ref, defprop-ref   B1: a closure that lists no read still runs one, through a value no call shows (a stored generator stepped by a spread, a Proxy trap and a
//                       getter installed through a handler or a descriptor declared apart), in a recognized computation (`rec`): the first build's callee reason ended `peek()` silently.
//                       The store-alias and props-alias shapes of B1 are not cells: the site would be a store or props read through an alias, which the checker lists as no read at all (S4 of the
//                       review, open-work), so the generator has no marked site to place there; the oracle case `after-await-closure` holds them.
//
// Recall contexts, one base shape (the direct binding): the question is how the computation is started and how the continuation reaches the read, not the spelling of the site. Not
// crossed with other codes by the census, out of api-fire's plan.
import { awaitStmt, call, contain, decl, domExpr, expr, fn, num, prim, ref, ret } from "./model.mjs";

export const CS_FORM_NAMES = ["wrapper", "call", "reflect", "element"];
/** The start the index recognizes: the compute written inline in the primitive. */
export const CS_RECOGNIZED = "rec";
export const CS_HOSTS = ["memo", "effect"];

/** What each host starts: the primitive (a `solid-js` export), its type as a function of one compute (and the effect's apply), and the arguments that follow the compute. */
const HOSTS = {
  memo: { primitive: "createMemo", type: "(compute: () => Promise<number>) => unknown", rest: "", wrapped: (compute) => `return createMemo(${compute})` },
  effect: { primitive: "createEffect", type: "(compute: () => Promise<number>, effect: () => void) => unknown", rest: ", () => {}", wrapped: (compute) => `createEffect(${compute}, () => {})` },
};

/** Each hidden start: `defs(n, w, host)` the module-scope text that precedes the start (`[]` for none), and the host-side statements `run` (starts the function `w`) and `idle` (names it, starts nothing). */
const FORMS = {
  wrapper: (n, w, host) => { const t = n("track"); return { defs: [`function ${t}(compute: () => Promise<number>) { ${host.wrapped("compute")}; }`], run: [`${t}(${w})`], idle: [`void ${t}`, `void ${w}`] }; },
  call: (n, w, host) => ({ defs: [], run: [`(${host.primitive} as ${host.type}).call(undefined, ${w}${host.rest})`], idle: [`void ${w}`] }),
  reflect: (n, w, host) => ({ defs: [], run: [`Reflect.apply(${host.primitive} as ${host.type}, undefined, [${w}${host.rest}])`], idle: [`void ${w}`] }),
  element: (n, w, host) => { const p = n("primitives"); return { defs: [], run: [`const ${p} = [${host.primitive} as ${host.type}]`, `${p}[0](${w}${host.rest})`], idle: [`void ${w}`] }; },
};

const text = (line) => expr(domExpr(line));

/**
 * The routes: `(n, stmts) => { hoisted, inside }`. `hoisted` is what the module declares (the helpers holding the marked site `stmts`), `inside` the statements the started function writes
 * where the continuation reaches the site (after its `await`, or ahead of it in the `-before` twin).
 */
const ROUTES = {
  "": (n, stmts) => ({ hoisted: [], inside: stmts }),
  getter: (n, stmts) => {
    const o = n("holder");
    const peek = n("peek");
    return { hoisted: [contain("frame", 9, stmts, { head: `const ${o} = { get value() `, close: " };", after: ["return 1;"] }), text(`function ${peek}() { return ${o}.value; }`)], inside: [text(`void ${peek}()`)] };
  },
  // B2: a member call on a module-scope object literal.
  "ns-method": (n, stmts) => { const o = n("ns"); return { hoisted: [contain("frame", 9, stmts, { head: `const ${o} = { run() `, close: " };" })], inside: [text(`${o}.run()`)] }; },
  "ns-arrow": (n, stmts) => { const o = n("ns"); return { hoisted: [contain("frame", 9, stmts, { head: `const ${o} = { run: () => `, close: " };" })], inside: [text(`${o}.run()`)] }; },
  "ns-computed": (n, stmts) => { const o = n("ns"); return { hoisted: [contain("frame", 9, stmts, { head: `const ${o} = { run() `, close: " };" })], inside: [text(`${o}["run"]()`)] }; },
  "ns-optional": (n, stmts) => { const o = n("ns"); return { hoisted: [contain("frame", 9, stmts, { head: `const ${o} = { run() `, close: " };" })], inside: [text(`${o}?.run()`)] }; },
  // B3: a standard call that runs a function value, and a function literal called where it is written.
  dotcall: (n, stmts) => { const r = n("reader"); return { hoisted: [contain("frame", 9, stmts, { head: `function ${r}() `, close: "" })], inside: [text(`${r}.call(undefined)`)] }; },
  apply: (n, stmts) => { const r = n("reader"); return { hoisted: [contain("frame", 9, stmts, { head: `function ${r}() `, close: "" })], inside: [text(`${r}.apply(undefined, [])`)] }; },
  reflect: (n, stmts) => { const r = n("reader"); return { hoisted: [contain("frame", 9, stmts, { head: `function ${r}() `, close: "" })], inside: [text(`Reflect.apply(${r}, undefined, [])`)] }; },
  iife: (n, stmts) => ({ hoisted: [], inside: [contain("frame", 9, stmts, { head: "(() => ", close: ")();" })] }),
  "iife-fn": (n, stmts) => ({ hoisted: [], inside: [contain("frame", 9, stmts, { head: "(function () ", close: ")();" })] }),
  "iife-async": (n, stmts) => ({ hoisted: [], inside: [contain("frame", 9, stmts, { head: "await (async () => ", close: ")();" })] }),
  // B1: a read the closure of `peek` does not show.
  gen: (n, stmts) => {
    const g = n("numbers");
    const it = n("stored");
    const peek = n("peek");
    return { hoisted: [contain("frame", 9, stmts, { head: `function* ${g}() `, close: "", after: ["yield 1;"] }), text(`const ${it} = ${g}()`), text(`function ${peek}() { return [...${it}]; }`)], inside: [text(`void ${peek}()`)] };
  },
  "proxy-ref": (n, stmts) => {
    const r = n("reader");
    const traps = n("traps");
    const proxy = n("proxy");
    const peek = n("peek");
    return {
      hoisted: [contain("frame", 9, stmts, { head: `function ${r}() `, close: "", after: ["return 1;"] }), text(`const ${traps} = { get: ${r} }`), text(`const ${proxy}: any = new Proxy({}, ${traps})`), text(`function ${peek}() { return ${proxy}.x; }`)],
      inside: [text(`void ${peek}()`)],
    };
  },
  "defprop-ref": (n, stmts) => {
    const r = n("reader");
    const desc = n("desc");
    const holder = n("installed");
    const peek = n("peek");
    return {
      hoisted: [contain("frame", 9, stmts, { head: `function ${r}() `, close: "", after: ["return 1;"] }), text(`const ${desc} = { get: ${r} }`), text(`const ${holder}: any = {}`), text(`Object.defineProperty(${holder}, "value", ${desc})`), text(`function ${peek}() { return ${holder}.value; }`)],
      inside: [text(`void ${peek}()`)],
    };
  },
};

/** The start forms each route is crossed with: the first two ask how the function is started, the others how the continuation reaches the site, over one hidden start or a recognized one. */
const ROUTE_FORMS = {
  "": CS_FORM_NAMES,
  getter: CS_FORM_NAMES,
  "ns-method": ["wrapper"], "ns-arrow": ["wrapper"], "ns-computed": ["wrapper"], "ns-optional": ["wrapper"],
  dotcall: ["wrapper"], apply: ["wrapper"], reflect: ["wrapper"], iife: ["wrapper"], "iife-fn": ["wrapper"], "iife-async": ["wrapper"],
  gen: [CS_RECOGNIZED], "proxy-ref": [CS_RECOGNIZED], "defprop-ref": [CS_RECOGNIZED],
};
export const CS_ROUTE_NAMES = Object.keys(ROUTES).filter((route) => route !== "");

/** `{ [context]: "misuse" | "legal" }`. */
export const CS_CONTEXTS = Object.fromEntries(
  Object.entries(ROUTE_FORMS).flatMap(([route, forms]) => forms.flatMap((form) => CS_HOSTS.flatMap((host) => {
    const base = `cs-${form}-${host}${route ? `-${route}` : ""}`;
    return [[base, "misuse"], [`${base}-never`, "legal"], [`${base}-before`, "legal"]];
  }))),
);
export const CS_CONTEXT_NAMES = Object.keys(CS_CONTEXTS);
export const CS_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

const byLength = (names) => [...names].sort((a, b) => b.length - a.length);
const NAME = new RegExp(`^cs-(${byLength([...CS_FORM_NAMES, CS_RECOGNIZED]).join("|")})-(${CS_HOSTS.join("|")})(?:-(${byLength(CS_ROUTE_NAMES).join("|")}))?(-never|-before)?$`);
/** `{ form, host, route, getter, never, before }` of a `cs-*` context name (`route` is `""` for the read written in the started function), or null. */
export const csParts = (context) => {
  const match = NAME.exec(context);
  if (!match) return null;
  const route = match[3] ?? "";
  if (!ROUTE_FORMS[route].includes(match[1])) return null;
  return { form: match[1], host: match[2], route, getter: route === "getter", never: match[4] === "-never", before: match[4] === "-before" };
};

/** The base shapes a `cs-*` cell takes: the direct binding, no layer. */
export function csShape(context, shape) {
  return !csParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The placement of `stmts` for a `cs-*` context (`{ body, view, hoisted }`), or undefined for any other. */
export function csContext(context, stmts, kit) {
  const parts = csParts(context);
  if (!parts) return undefined;
  const w = kit.name("work");
  const host = HOSTS[parts.host];
  const route = ROUTES[parts.route](kit.name, stmts);
  // The started function: `async function work() { await ...; <where the continuation reaches the site>; return 1; }`, or ahead of the `await` in the `-before` twin.
  const inside = parts.before ? [...route.inside, awaitStmt(), ret(num(1))] : [awaitStmt(), ...route.inside, ret(num(1))];
  // A line that names the primitive imports it from `solid-js`; one that does not (`void work`) leaves no unused import behind.
  const line = (value) => expr(value.includes(host.primitive) ? domExpr(value, undefined, [host.primitive]) : domExpr(value));
  if (parts.form === CS_RECOGNIZED) {
    // A recognized start: the compute is the arrow handed to the primitive. The `-never` twin declares the same function and starts nothing.
    if (parts.never) return { body: [line(`void ${w}`)], view: [], hoisted: [...route.hoisted, decl(w, fn(inside, { form: "function", async: true }))] };
    const compute = fn(inside, { async: true });
    if (parts.host === "memo") {
      const m = kit.name("m");
      return { body: [decl(m, prim("memo", compute)), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))], view: [], hoisted: route.hoisted };
    }
    return { body: [expr(prim("effect", compute, { apply: fn([]) }))], view: [], hoisted: route.hoisted };
  }
  const form = FORMS[parts.form](kit.name, w, host);
  return { body: (parts.never ? form.idle : form.run).map(line), view: [], hoisted: [...route.hoisted, decl(w, fn(inside, { form: "function", async: true })), ...form.defs.map(line)] };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function csPlacement() {
  const names = CS_CONTEXT_NAMES;
  return Object.fromEntries(CS_CODES.map((code) => [code, { misuse: names.filter((name) => CS_CONTEXTS[name] === "misuse"), legal: names.filter((name) => CS_CONTEXTS[name] === "legal") }]));
}
