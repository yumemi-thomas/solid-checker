// The JS-semantics axis (ADR 1117, the soundness-hardening milestone): odd JavaScript that runs project code where no call is written, injected into the rules' contexts.
// The checker's walks follow calls, reads and member reads; a Proxy trap, a getter, a coercion hook, `Symbol.hasInstance`, a computed key's `toString`, a `for…in` over a
// proxy and a tagged template each run a function with no call at the site. Each cell holds the code's own site in a function literal (`jo.f`, as pu-shapes.mjs's
// `pu-bare-*` write it) that no call names, and reaches it through one such route, written after an async memo's `await`, where rc.21 reports the hook's read (UNTRACKED_READ_AFTER_AWAIT). A route the checker does not
// see as a call is a site it must still account for: a missing row is a false clean, the reason this axis exists (ADR 1113 found `${host}` and `"x" in px` that way).
//
//   js-proxy-get    `jr.k` on `new Proxy({}, { get: jo.f })`
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
import { awaitStmt, call, decl, domExpr, expr, fn, num, obj, prim, ref, ret } from "./model.mjs";

/** Each cell: the value its hook returns, the declarations that install the hook (given `f`, the hook function's reference text, and `n`, the route's name), and the
 * statement the memo runs after its await. */
const ROUTES = {
  "js-proxy-get": ["1", (f, n) => [`const ${n} = new Proxy({} as Record<string, number>, { get: ${f} })`], (n) => `void ${n}.k`],
  "js-proxy-has": ["false", (f, n) => [`const ${n} = new Proxy({}, { has: ${f} })`], (n) => `void ("k" in ${n})`],
  "js-proxy-keys": ["[]", (f, n) => [`const ${n} = new Proxy({}, { ownKeys: ${f} })`], (n) => `void Object.keys(${n})`],
  "js-proxy-proto": ["Object.prototype", (f, n) => [`const ${n} = new Proxy({}, { getPrototypeOf: ${f} })`], (n) => `void (${n} instanceof Object)`],
  "js-forin": ["[]", (f, n) => [`const ${n} = new Proxy({}, { ownKeys: ${f} })`], (n) => `for (const k in ${n}) void k`],
  "js-getter": ["1", (f, n) => [`const ${n} = Object.defineProperty({} as { v?: number }, "v", { get: ${f} })`], (n) => `void ${n}.v`],
  "js-tostring": ['""', (f, n) => [`const ${n} = { toString: ${f} }`], (n) => "void `${" + n + "}`"],
  "js-valueof": ["0", (f, n) => [`const ${n} = { valueOf: ${f} }`], (n) => `void ((${n} as unknown as number) < 1)`],
  "js-toprim": ['""', (f, n) => [`const ${n} = { [Symbol.toPrimitive]: ${f} }`], (n) => "void `${" + n + "}`"],
  "js-hasinst": ["false", (f, n) => [`const ${n} = { [Symbol.hasInstance]: ${f} } as unknown as new () => object`], (n) => `void ({} instanceof ${n})`],
  "js-computed": ['"k"', (f, n) => [`const ${n} = { toString: ${f} }`, `const ${n}t: Record<string, number> = {}`], (n) => `void ${n}t[${n} as unknown as string]`],
  "js-tagged": ['""', (f, n) => [`const ${n} = ${f}`], (n) => `void ${n}\`x\``],
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

/**
 * The placement of `stmts` (the code's own site) for a `js-*` context (`{ body, view }`), or undefined for any other. The hook is a function literal holding the site, kept
 * as a member of a literal (`jo.f`) and installed by reference, so no call to it is written anywhere: the route alone runs it.
 */
export function jsContext(context, stmts, kit) {
  if (!JS_CONTEXT_NAMES.includes(context)) return undefined;
  const [returned, install, run] = ROUTES[context];
  const holder = kit.name("jo");
  const route = kit.name("jr");
  const m = kit.name("m");
  const memo = decl(m, prim("memo", fn([awaitStmt(), expr(domExpr(run(route))), ret(num(1))], { async: true })));
  return {
    body: [decl(holder, obj([["f", fn([...stmts, ret(domExpr(returned))])]])), ...install(`${holder}.f`, route).map((text) => expr(domExpr(text))), memo, expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))],
    view: [],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function jsPlacement() {
  return Object.fromEntries(JS_CODES.map((code) => [code, { misuse: JS_CONTEXT_NAMES, legal: [] }]));
}
