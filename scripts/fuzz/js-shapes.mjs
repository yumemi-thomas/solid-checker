// The JS-semantics axis (ADR 1117, the soundness-hardening milestone): odd JavaScript that runs project code where no call is written, injected into the rules' contexts.
// The checker's walks follow calls, reads and member reads; a Proxy trap, a getter, a coercion hook, `Symbol.hasInstance`, a computed key's `toString`, a `for…in` over a
// proxy and a tagged template each run a function with no call at the site. Each cell holds the code's own site in a helper (`jh`, module scope, as pu-shapes.mjs writes it)
// and reaches it through one such route, written after an async memo's `await`, where rc.21 reports the helper's read (UNTRACKED_READ_AFTER_AWAIT). A route the checker does not
// see as a call is a site it must still account for: a missing row is a false clean, the reason this axis exists (ADR 1113 found `${host}` and `"x" in px` that way).
//
//   js-proxy-get    `jp.k` on `new Proxy({}, { get() { jh(); .. } })`
//   js-proxy-has    `"k" in jp`, the `has` trap
//   js-proxy-keys   `Object.keys(jp)`, the `ownKeys` trap
//   js-proxy-proto  `jp instanceof Object`, the `getPrototypeOf` trap (a standard right side)
//   js-forin        `for (const k in jp)`, the `ownKeys` trap
//   js-getter       `jo.v`, an accessor property of a literal
//   js-tostring     `${jo}`, `toString`
//   js-valueof      `jo < 1`, `valueOf` (cast through `unknown`, so tsc accepts it)
//   js-toprim       `${jo}`, `Symbol.toPrimitive`
//   js-hasinst      `{} instanceof JK`, a static `Symbol.hasInstance`
//   js-computed     `jt[jk]`, a computed key whose `toString` runs
//   js-tagged       `` jg`x` ``, a tagged template (a call written without parentheses)
//
// Seeds still to write (ADR 1117 split): patched standard and DOM prototypes (a getPrototypeOf route, `EventTarget.prototype.addEventListener`), shadow DOM delivery, escaped
// or concatenated keys, cross-module re-exports (default, star, namespace, barrel), and the review probes of ADRs 1097, 1102b, 1089 E, 1108 and 1098 D.
// Recall contexts, the direct binding bare (`jsShape`), one program per cell, not crossed by the census, out of api-fire's plan.
import { awaitStmt, call, decl, domExpr, expr, fn, num, prim, ref, ret } from "./model.mjs";

/** Each cell: the module-scope declarations that hold the route (given the helper's name), and the expression the memo runs after its await. */
const ROUTES = {
  "js-proxy-get": (h, n) => [[`const ${n} = new Proxy({} as Record<string, number>, { get() { ${h}(); return 1; } })`], `void ${n}.k`],
  "js-proxy-has": (h, n) => [[`const ${n} = new Proxy({}, { has() { ${h}(); return false; } })`], `void ("k" in ${n})`],
  "js-proxy-keys": (h, n) => [[`const ${n} = new Proxy({}, { ownKeys() { ${h}(); return []; } })`], `void Object.keys(${n})`],
  "js-proxy-proto": (h, n) => [[`const ${n} = new Proxy({}, { getPrototypeOf() { ${h}(); return Object.prototype; } })`], `void (${n} instanceof Object)`],
  "js-forin": (h, n) => [[`const ${n} = new Proxy({}, { ownKeys() { ${h}(); return []; } })`], `for (const k in ${n}) void k`],
  "js-getter": (h, n) => [[`const ${n} = { get v() { ${h}(); return 1; } }`], `void ${n}.v`],
  "js-tostring": (h, n) => [[`const ${n} = { toString() { ${h}(); return ""; } }`], "void `${" + n + "}`"],
  "js-valueof": (h, n) => [[`const ${n} = { valueOf() { ${h}(); return 0; } }`], `void ((${n} as unknown as number) < 1)`],
  "js-toprim": (h, n) => [[`const ${n} = { [Symbol.toPrimitive]() { ${h}(); return ""; } }`], "void `${" + n + "}`"],
  "js-hasinst": (h, n) => [[`class ${n} { static [Symbol.hasInstance](_: unknown) { ${h}(); return false; } }`], `void ({} instanceof ${n})`],
  "js-computed": (h, n) => [[`const ${n} = { toString() { ${h}(); return "k"; } }`, `const ${n}t: Record<string, number> = {}`], `void ${n}t[${n} as unknown as string]`],
  "js-tagged": (h, n) => [[`const ${n} = (s: TemplateStringsArray) => { ${h}(); return s; }`], `void ${n}\`x\``],
};

export const JS_CONTEXTS = Object.fromEntries(Object.keys(ROUTES).map((name) => [name, "misuse"]));
export const JS_CONTEXT_NAMES = Object.keys(JS_CONTEXTS);
export const JS_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

/** Who writes and runs each cell (ADR 1117's split between W4 and W3): these twelve are W4's; W3 takes the seeds listed above. */
export const JS_OWNER = Object.fromEntries(JS_CONTEXT_NAMES.map((name) => [name, "W4"]));

/** The base shapes a `js-*` cell takes: the direct binding, no layer. */
export function jsShape(context, shape) {
  return !JS_CONTEXT_NAMES.includes(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The placement of `stmts` (the code's own site) for a `js-*` context (`{ hoisted, body, view }`), or undefined for any other. */
export function jsContext(context, stmts, kit) {
  if (!JS_CONTEXT_NAMES.includes(context)) return undefined;
  const helper = kit.name("jh");
  const route = kit.name("jr");
  const [declarations, run] = ROUTES[context](helper, route);
  const m = kit.name("m");
  const memo = decl(m, prim("memo", fn([awaitStmt(), expr(domExpr(run)), ret(num(1))], { async: true })));
  return {
    hoisted: [decl(helper, fn(stmts)), ...declarations.map((text) => expr(domExpr(text)))],
    body: [memo, expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))],
    view: [],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function jsPlacement() {
  return Object.fromEntries(JS_CODES.map((code) => [code, { misuse: JS_CONTEXT_NAMES, legal: [] }]));
}
