// A primitive handed BY REFERENCE (ADR 0990, batch 11 slices 5 and 7): the site is `flush`, `resolve` or `until` named as a value, never written as a call at the site.
//
// The other cells hand a callback to a primitive (`pl-*`) or run a function the primitive's code sits in (`nm-*`, `la-*`, `ob-*`). Here the primitive itself is the thing handed
// around, and the call is made by something else: `untrack(flush)` calls it, an alias calls it, an array element calls it, a project helper calls its parameter. The checker answers
// each of them with an SC9012 hand-off at the exact site, a proof of the rule, or an uncertifiable row of the rule that asked; the runtime does not care how the name is spelled, so
// Chrome fires the code wherever the plain call would. Never silence.
//
//   fr-<form>-<host>          misuse   the site is the primitive handed by reference in the host
//   fr-<form>-<host>-never    legal    the same text inside a function nothing calls (the host's callback only names it): no site runs, Chrome reports nothing
//     forms   untrack   `untrack(flush)`                                  (`flush` only: `resolve` and `until` take a required argument, and `untrack(resolve)` is a type error)
//             alias     `const f = flush; f()`, the alias written in the host's callback just above the call
//             element   `[flush][0]()`
//             param     `frCall(flush)`, a module-scope helper that calls its parameter (`function frCall(f: () => unknown) { f(); }`)
//     hosts   action (an action's first step, called from a button: FLUSH_IN_ACTION), tracked (`createTrackedEffect`: FLUSH_IN_TRACKED_QUEUE_CALLBACK), effect-apply
//             (an effect's apply: FLUSH_IN_EFFECT_CALLBACK), memo (a memo's compute: RESOLVE_IN_REACTIVE_SCOPE and UNTIL_IN_REACTIVE_SCOPE)
//
// Recall contexts, the direct binding bare (`frShape`), one program per cell, not crossed by the census. Chrome's verdict is what the judge compares the checker with: a `fr-*` misuse
// cell it keeps quiet on would be `not-fired`, never a false clean (the run's counts say so: scripts/fuzz/README.md).
import { decl, domExpr, expr, fn } from "./model.mjs";

export const FR_FORMS = ["untrack", "alias", "element", "param"];
/** The primitive each code is about, the hosts it takes, and the forms that type-check for it. */
export const FR_CODES = {
  FLUSH_IN_ACTION: { primitive: "flush", hosts: ["action"], forms: FR_FORMS },
  FLUSH_IN_TRACKED_QUEUE_CALLBACK: { primitive: "flush", hosts: ["tracked"], forms: FR_FORMS },
  FLUSH_IN_EFFECT_CALLBACK: { primitive: "flush", hosts: ["effect-apply"], forms: FR_FORMS },
  RESOLVE_IN_REACTIVE_SCOPE: { primitive: "resolve", hosts: ["memo"], forms: FR_FORMS.filter((form) => form !== "untrack") },
  UNTIL_IN_REACTIVE_SCOPE: { primitive: "until", hosts: ["memo"], forms: FR_FORMS.filter((form) => form !== "untrack") },
};
export const FR_HOSTS = ["action", "tracked", "effect-apply", "memo"];
const contextsOf = (code) => FR_CODES[code].hosts.flatMap((host) => FR_CODES[code].forms.flatMap((form) => [`fr-${form}-${host}`, `fr-${form}-${host}-never`]));
/** `{ [context]: "misuse" | "legal" }`. */
export const FR_CONTEXTS = Object.fromEntries(Object.keys(FR_CODES).flatMap((code) => contextsOf(code).map((context) => [context, context.endsWith("-never") ? "legal" : "misuse"])));
export const FR_CONTEXT_NAMES = Object.keys(FR_CONTEXTS);

const NAME = new RegExp(`^fr-(${FR_FORMS.join("|")})-(${FR_HOSTS.join("|")})(-never)?$`);
/** `{ form, host, never }` of a `fr-*` context name, or null. */
export const frParts = (context) => {
  const match = NAME.exec(context);
  return match ? { form: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes a `fr-*` cell takes: the direct binding, no layer. */
export function frShape(context, shape) {
  return !frParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/**
 * The placement of `stmts` (the code's own site, one `use` statement) for a `fr-*` context, or undefined for any other. The site keeps its node; `fr` on it tells the printer
 * (misuse.mjs `describeProjectUse`) to spell the call the form's way. `place(host, inner)` is the plain host's own placement.
 */
export function frContext(context, stmts, kit, place) {
  const parts = frParts(context);
  if (!parts) return undefined;
  const use = stmts.length === 1 && stmts[0].k === "expr" && stmts[0].e.k === "use" ? stmts[0].e : null;
  const primitive = use && FR_CODES[use.code]?.primitive;
  if (!primitive) throw new Error(`${context}: the site is not a call of flush, resolve or until`);
  const callee = parts.form === "alias" ? kit.name("fa") : null;
  const site = { ...use, fr: { form: parts.form, callee } };
  const pre = callee ? [decl(callee, domExpr(primitive, undefined, [primitive]))] : [];
  const hoisted = parts.form === "param" ? [expr(domExpr(`function frCall(f: ${primitive === "flush" ? "() => unknown" : "(a: () => number) => unknown"}) { f(${primitive === "flush" ? "" : "() => 1"}); }`))] : [];
  const held = [...pre, expr(site)];
  if (!parts.never) return { ...place(parts.host, held), ...(hoisted.length ? { hoisted } : {}) };
  // The twin: the same text in a function the host's callback only names.
  const never = kit.name("fz");
  return { ...place(parts.host, [decl(never, fn(held)), expr(domExpr(`void ${never}`))]), ...(hoisted.length ? { hoisted } : {}) };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function frPlacement() {
  return Object.fromEntries(Object.keys(FR_CODES).map((code) => {
    const contexts = contextsOf(code);
    return [code, { misuse: contexts.filter((context) => FR_CONTEXTS[context] === "misuse"), legal: contexts.filter((context) => FR_CONTEXTS[context] === "legal") }];
  }));
}
