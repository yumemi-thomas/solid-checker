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

/**
 * The members (ADR 0939): `isfinite` replaces `Number.isFinite` outright (a value-only row; the runtime never calls it). `then` and
 * `settimeout` are members Solid and the host call themselves, so their replacement wraps the original and delegates to it for every
 * function but the program's own, which it runs at once: `Promise.prototype.then` (the twin wraps `catch`) and the global `setTimeout`
 * (the twin wraps `setInterval`). Each is called in the body with the function holding the site.
 */
const MEMBERS = {
  isfinite: {
    root: "Number",
    key: "isFinite",
    otherKey: "isNaN",
    prelude: () => [],
    impl: (f) => `(f: any) => { if (typeof f === "function" && f.name === "${f}") { f(); return true; } return false; }`,
    call: (f) => `Number.isFinite(${f} as unknown as number)`,
  },
  then: {
    root: "Promise.prototype",
    key: "then",
    otherKey: "catch",
    prelude: (orig, key) => [decl(orig, domExpr(`Promise.prototype.${key} as (...args: any[]) => any`))],
    impl: (f, orig) => `function (this: any, a: any, b: any) { if (typeof a === "function" && a.name === "${f}") { a(); return this; } return ${orig}.call(this, a, b); }`,
    call: (f) => `Promise.resolve(0).then(${f})`,
  },
  settimeout: {
    root: "globalThis",
    key: "setTimeout",
    otherKey: "setInterval",
    prelude: (orig, key) => [decl(orig, domExpr(`globalThis.${key}.bind(globalThis)`))],
    impl: (f, orig) => `(cb: any, ms?: number) => { if (typeof cb === "function" && cb.name === "${f}") { cb(); return 0; } return ${orig}(cb, ms); }`,
    call: (f) => `setTimeout(${f}, 0)`,
  },
};

/** `{ [context]: { member, form, other } }`. */
export const SEAM_CONTEXTS = {};
for (const member of Object.keys(MEMBERS)) {
  for (const form of Object.keys(FORMS)) {
    SEAM_CONTEXTS[`bseam-${member}-${form}`] = { member, form, other: false };
    SEAM_CONTEXTS[`bseam-${member}-${form}-other`] = { member, form, other: true };
  }
}
export const SEAM_CONTEXT_NAMES = Object.keys(SEAM_CONTEXTS);
export const SEAM_MEMBERS = Object.keys(MEMBERS);

/**
 * The placement of `stmts` for a `bseam-*` context (`{ body, view }`), or undefined for any other. The helper is declared in the body of the component, the
 * replacement installed there, and the member called with a function holding the site: all in the body's own window.
 */
export function seamContext(context, stmts, kit) {
  const spec = SEAM_CONTEXTS[context];
  if (!spec) return undefined;
  const member = MEMBERS[spec.member];
  const helper = kit.name("patch");
  const f = kit.name("sf");
  const key = spec.other ? member.otherKey : member.key;
  const orig = member.prelude.length ? kit.name("orig") : undefined;
  return {
    body: [
      decl(helper, domExpr(`(target: any, key: string, impl: unknown) => { ${FORMS[spec.form]} }`)),
      ...member.prelude(orig, key),
      expr(domExpr(`${helper}(${member.root}, "${key}", ${member.impl(f, orig)})`)),
      decl(f, fn(stmts)),
      expr(domExpr(member.call(f))),
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
