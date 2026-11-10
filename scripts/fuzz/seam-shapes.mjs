// The seam shapes (ADR 0935): a case that two of the checker's checks each leave to the other.
//
// The first one found was the iteration protocol (ADR 0803): the census (`handed_to_unseen_code`, `handed_on`) treated a mutating builtin as the visible-write
// scan's question, and the scan decided only where the target or the key is spelled at the mutation. A project helper handed `Array.prototype` that
// writes its parameter under an unspelled key belonged to neither, and a patched iterator gave a certified false clean. The pattern is general: wherever a
// premise says "the standard library behaves as its declaration says", a program can replace the member through a helper that takes the intrinsic as a
// parameter (`Object.assign(target, { [key]: impl })`, `Reflect.set`, an index write, `Object.defineProperty`), and no spelling at the call names it.
//
// A seam context replaces `Number.isFinite`, a value-only row (the API never runs a function argument), by a function that runs the program's own function
// and calls the member with it; its twin installs the same function under another key (`Number.isNaN`), so `isFinite` keeps its declared behaviour and the
// function never runs. They differ only in the key: the fact the premise claims, "`Number.isFinite` is the declared `Number.isFinite`".
//
//   bseam-isfinite-<form>          misuse  the member is replaced; the function runs at once, in the window of the call
//   bseam-isfinite-<form>-other    legal   the replacement goes to another member; the declared one is called
//
// forms: assign, reflect, index, define (the ways a helper writes its parameter). Only a member the runtime does not itself call can be replaced in a program
// the Chrome host runs: the replacement does not read the original (a program that does is already uncertifiable, SC9012 `standard-member-impostor`), so it would
// break Solid and the host if it were `Set.add`, `setTimeout`, `Promise.prototype.then` or `addEventListener`. Those were scanned by the checker alone and are
// reported, not shaped. Recall contexts, base shapes (misuse.mjs BASE_ONLY_CONTEXTS), on the three codes the hand-off roles decide.
import { decl, domExpr, expr, fn } from "./model.mjs";

const FORMS = {
  assign: "Object.assign(target, { [key]: impl });",
  reflect: "Reflect.set(target, key, impl);",
  index: "target[key] = impl;",
  define: "Object.defineProperty(target, key, { value: impl, configurable: true, writable: true });",
};

/** `{ [context]: { form, other } }`. */
export const SEAM_CONTEXTS = {};
for (const form of Object.keys(FORMS)) {
  SEAM_CONTEXTS[`bseam-isfinite-${form}`] = { form, other: false };
  SEAM_CONTEXTS[`bseam-isfinite-${form}-other`] = { form, other: true };
}
export const SEAM_CONTEXT_NAMES = Object.keys(SEAM_CONTEXTS);

/**
 * The placement of `stmts` for a `bseam-*` context (`{ body, view }`), or undefined for any other. The helper is declared in the body of the component, the
 * replacement installed there, and the member called with a function holding the site: all in the body's own window.
 */
export function seamContext(context, stmts, kit) {
  const spec = SEAM_CONTEXTS[context];
  if (!spec) return undefined;
  const helper = kit.name("patch");
  const f = kit.name("sf");
  const key = spec.other ? "isNaN" : "isFinite";
  const impl = `(f: any) => { if (typeof f === "function" && f.name === "${f}") { f(); return true; } return false; }`;
  return {
    body: [
      decl(helper, domExpr(`(target: any, key: string, impl: unknown) => { ${FORMS[spec.form]} }`)),
      expr(domExpr(`${helper}(Number, "${key}", ${impl})`)),
      decl(f, fn(stmts)),
      expr(domExpr(`Number.isFinite(${f} as unknown as number)`)),
    ],
    view: [],
  };
}

/** `{ [code]: { misuse, legal } }` for SPEC: the replaced member runs the function in the body's window, the other member leaves the declared one to ignore it. */
export function seamPlacement() {
  const placement = {};
  for (const code of ["STRICT_READ_UNTRACKED", "REACTIVE_WRITE_IN_OWNED_SCOPE", "ACTION_CALLED_IN_OWNED_SCOPE"]) {
    placement[code] = { misuse: SEAM_CONTEXT_NAMES.filter((name) => !SEAM_CONTEXTS[name].other), legal: SEAM_CONTEXT_NAMES.filter((name) => SEAM_CONTEXTS[name].other) };
  }
  return placement;
}

/** The forms, for the registry's pairs. */
export const SEAM_FORMS = Object.keys(FORMS);
