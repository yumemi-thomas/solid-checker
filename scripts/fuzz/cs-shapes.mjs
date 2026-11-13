// The computation-start shapes (batch 11, slice 9B, ADR 0973): the site is the read after the `await` of an async FUNCTION DECLARATION that the program starts as a tracked async
// computation through a form the producers' index of computations does not follow. Chrome (rc.21) runs the function as the compute of `createMemo` or `createEffect` and reports
// UNTRACKED_READ_AFTER_AWAIT at the read, while a program with no computation the index finds was clean on main 177402dfb (the oracle cases `after-await-universe-census-*`: no SC1002
// row at the read; SC1001's `function-run-unplaced` row beside it claims another code). The accounting of slice 9B (`after_await_accounting.rs`) keeps SC1002's uncertifiable row there.
//
//   cs-<form>-<host>          misuse   the function the form starts runs as the host's compute: the read after its `await` is untracked
//   cs-<form>-<host>-never    legal    the same text, nothing starts the function (the names stay used)
//   cs-<form>-<host>-before   legal    the same program with the read ahead of the `await`: a dependency of the computation, Chrome reports nothing
//
//   forms: wrapper  `function track(compute) { return createMemo(compute); }`, `track(work)`: the producers' hand-off (ADR 0873) follows a function literal and an arrow through such a
//                   wrapper, not a function declaration handed by identifier
//          call     `(createMemo as ...).call(undefined, work)`
//          reflect  `Reflect.apply(createMemo as ..., undefined, [work])`
//          element  `[createMemo as ...][0](work)`
//   hosts: `memo` (`createMemo`) and `effect` (`createEffect`'s compute), SC1002's two async computes.
//
// Recall contexts, one base shape (the direct binding): the question is how the computation is started, not the spelling of the site. Not crossed with other codes by the census, out of
// api-fire's plan.
import { awaitStmt, decl, domExpr, expr, fn, num, ret } from "./model.mjs";

export const CS_FORM_NAMES = ["wrapper", "call", "reflect", "element"];
export const CS_HOSTS = ["memo", "effect"];

/** What each host starts: the primitive (a `solid-js` export), its type as a function of one compute (and the effect's apply), and the arguments that follow the compute. */
const HOSTS = {
  memo: { primitive: "createMemo", type: "(compute: () => Promise<number>) => unknown", rest: "", wrapped: (compute) => `return createMemo(${compute})` },
  effect: { primitive: "createEffect", type: "(compute: () => Promise<number>, effect: () => void) => unknown", rest: ", () => {}", wrapped: (compute) => `createEffect(${compute}, () => {})` },
};

/** Each form: `defs(n, w, host)` the module-scope text that precedes the start (`[]` for none), and the host-side statements `run` (starts the function `w`) and `idle` (names it, starts nothing). */
const FORMS = {
  wrapper: (n, w, host) => { const t = n("track"); return { defs: [`function ${t}(compute: () => Promise<number>) { ${host.wrapped("compute")}; }`], run: [`${t}(${w})`], idle: [`void ${t}`, `void ${w}`] }; },
  call: (n, w, host) => ({ defs: [], run: [`(${host.primitive} as ${host.type}).call(undefined, ${w}${host.rest})`], idle: [`void ${w}`] }),
  reflect: (n, w, host) => ({ defs: [], run: [`Reflect.apply(${host.primitive} as ${host.type}, undefined, [${w}${host.rest}])`], idle: [`void ${w}`] }),
  element: (n, w, host) => { const p = n("primitives"); return { defs: [], run: [`const ${p} = [${host.primitive} as ${host.type}]`, `${p}[0](${w}${host.rest})`], idle: [`void ${w}`] }; },
};

/** `{ [context]: "misuse" | "legal" }`. */
export const CS_CONTEXTS = Object.fromEntries(CS_FORM_NAMES.flatMap((form) => CS_HOSTS.flatMap((host) => [[`cs-${form}-${host}`, "misuse"], [`cs-${form}-${host}-never`, "legal"], [`cs-${form}-${host}-before`, "legal"]])));
export const CS_CONTEXT_NAMES = Object.keys(CS_CONTEXTS);
export const CS_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

const NAME = new RegExp(`^cs-(${[...CS_FORM_NAMES].sort((a, b) => b.length - a.length).join("|")})-(${CS_HOSTS.join("|")})(-never|-before)?$`);
/** `{ form, host, never, before }` of a `cs-*` context name, or null. */
export const csParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: match[3] === "-never", before: match[3] === "-before" } : null;
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
  const form = FORMS[parts.form](kit.name, w, host);
  // The started function: a declaration (`async function work() { await ...; <site>; return 1; }`), the read behind its `await`, or ahead of it in the `-before` twin.
  const body = parts.before ? [...stmts, awaitStmt(), ret(num(1))] : [awaitStmt(), ...stmts, ret(num(1))];
  const started = decl(w, fn(body, { form: "function", async: true }));
  // A line that names the primitive imports it from `solid-js`; one that does not (`void work`) leaves no unused import behind.
  const text = (line) => expr(line.includes(host.primitive) ? domExpr(line, undefined, [host.primitive]) : domExpr(line));
  return { body: (parts.never ? form.idle : form.run).map(text), view: [], hoisted: [started, ...form.defs.map(text)] };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function csPlacement() {
  const names = CS_CONTEXT_NAMES;
  return Object.fromEntries(CS_CODES.map((code) => [code, { misuse: names.filter((name) => CS_CONTEXTS[name] === "misuse"), legal: names.filter((name) => CS_CONTEXTS[name] === "legal") }]));
}
