// A NAMED callback of a primitive that runs it in place and takes what it throws (`isPending(cb)`), the shape the batch-11 research briefed (slice 5) and Chrome
// (rc.21, the real oracle) confirmed a false clean on main 3b22442d4: `const cb = async () => { flush(); }; createTrackedEffect(() => { n(); isPending(cb); })`. The
// literal `isPending(async () => flush())` is ADR 0959's (an async callback's synchronous throw is a rejection `isPending` does not catch: FLUSH_IN_TRACKED_QUEUE_CALLBACK);
// the named callback went through another reader (`slot_callback_operations`), which took the slot's swallowing for every callback.
//
//   nm-<form>-<host>   the site is the body of a function the program named at module scope and the host's callback hands to `isPending`
//     forms   async        `const cb = async () => { <site> }; isPending(cb)`                     misuse   the throw is a rejection, which `isPending` does not take
//             wrap-async   `isPending(() => cb())` over the same `cb`                              misuse   the arrow returns the rejected promise
//             async-after  `const cb = async () => { await …; <site> }; isPending(cb)`            legal    the site runs after the callback returned its promise
//             sync         `const cb = () => { <site> }; isPending(cb)`                            legal    the throw is the callback's own, which `isPending` takes
//             wrap-sync    `isPending(() => cb())` over a sync `cb`                                legal    the same, one call down
//             gen          `function* cb() { <site> }; isPending(cb)`                              legal    the call makes an iterator and runs no body
//   hosts   tracked (`createTrackedEffect`), settled (`onSettled`): SC3001's two leaf owners, the code FLUSH_IN_TRACKED_QUEUE_CALLBACK.
//
// What Chrome reports in each cell is what the judge compares the checker with (a `-misuse` cell Chrome keeps quiet on is no false clean). Recall contexts, the direct binding
// bare (`nmShape`), not crossed by the census, out of api-fire's plan.
import { awaitStmt, decl, domExpr, expr, fn } from "./model.mjs";

export const NM_FORMS = ["async", "wrap-async", "async-after", "sync", "wrap-sync", "gen"];
export const NM_HOSTS = ["tracked", "settled"];
export const NM_CODES = ["FLUSH_IN_TRACKED_QUEUE_CALLBACK"];
const MISUSE_FORMS = new Set(["async", "wrap-async"]);
/** `{ [context]: "misuse" | "legal" }`. */
export const NM_CONTEXTS = Object.fromEntries(NM_FORMS.flatMap((form) => NM_HOSTS.map((host) => [`nm-${form}-${host}`, MISUSE_FORMS.has(form) ? "misuse" : "legal"])));
export const NM_CONTEXT_NAMES = Object.keys(NM_CONTEXTS);

const NAME = new RegExp(`^nm-(${[...NM_FORMS].sort((a, b) => b.length - a.length).join("|")})-(${NM_HOSTS.join("|")})$`);
/** `{ form, host }` of an `nm-*` context name, or null. */
export const nmParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2] } : null;
};

/** The base shapes an `nm-*` cell takes: the direct binding, no layer. */
export function nmShape(context, shape) {
  return !nmParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The named function of a form: `stmts` is its body (after an `await` for `async-after`). */
const named = (form, h, stmts) => {
  switch (form) {
    case "async": case "wrap-async": return decl(h, fn(stmts, { async: true }));
    case "async-after": return decl(h, fn([awaitStmt(), ...stmts], { async: true }));
    case "sync": case "wrap-sync": return decl(h, fn(stmts));
    case "gen": return decl(h, fn(stmts, { form: "generator" }));
    default: throw new Error(`no named-callback form ${form}`);
  }
};

/** The placement of `stmts` for an `nm-*` context (`{ body, view, hoisted }`; `place(host, inner)` is the plain host's own placement), or undefined for any other. */
export function nmContext(context, stmts, kit, place) {
  const parts = nmParts(context);
  if (!parts) return undefined;
  const h = kit.name("cb");
  const handed = parts.form.startsWith("wrap") ? `isPending(() => ${h}())` : `isPending(${h})`;
  return { ...place(parts.host, [expr(domExpr(handed, undefined, ["isPending"]))]), hoisted: [named(parts.form, h, stmts)] };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function nmPlacement() {
  const of = (side) => NM_CONTEXT_NAMES.filter((name) => NM_CONTEXTS[name] === side);
  return Object.fromEntries(NM_CODES.map((code) => [code, { misuse: of("misuse"), legal: of("legal") }]));
}
