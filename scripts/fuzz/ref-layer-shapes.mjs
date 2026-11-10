// The ref-layer shapes (ADR 0935): a layer that runs in place inside an intrinsic `ref` callback, before an await of the callback or after one.
//
// rc.17's `ref` applies the callback while the element is created (`@solidjs/web` `dist/web.dev.js:1016-1022`), in the strict-read window of the code
// creating it; a callback that awaits first continues on a later task, in no window. The read in a layer the callback calls (an IIFE, an inline
// `forEach` callback) belongs to the callback's stack, so it is in the window before the first await and out of it after. The arm that places it is
// `execution_role.ref_applied_read_role.unreported` (execution_role.rs:552); no program reached it before these.
//
//   barm-ref-iife-sync    misuse  `ref={(el) => { (() => { read })(); }}`
//   barm-ref-iife-await   legal   `ref={async (el) => { await …; (() => { read })(); }}`
//   barm-ref-each-sync    misuse  `ref={(el) => { [1].forEach(() => { read }); }}`
//   barm-ref-each-await   legal   `ref={async (el) => { await …; [1].forEach(() => { read }); }}`
//
// Recall contexts only, base shapes (misuse.mjs BASE_ONLY_CONTEXTS), on the read code: a write in a ref callback is a different premise.
import { awaitStmt, expr, fn, iife, iter, refEl } from "./model.mjs";

/** `{ [context]: "misuse" | "legal" }`. */
export const REF_LAYER_CONTEXTS = { "barm-ref-iife-sync": "misuse", "barm-ref-iife-await": "legal", "barm-ref-each-sync": "misuse", "barm-ref-each-await": "legal" };
export const REF_LAYER_CONTEXT_NAMES = Object.keys(REF_LAYER_CONTEXTS);

/** The placement of `stmts` for a `barm-ref-*` context (`{ body, view }`), or undefined for any other. */
export function refLayerContext(context, stmts, kit) {
  if (!REF_LAYER_CONTEXTS[context]) return undefined;
  const layer = context.includes("-iife-") ? expr(iife(fn(stmts))) : expr(iter("forEach", fn(stmts)));
  const callback = context.endsWith("-await") ? fn([awaitStmt(), layer], { async: true }) : fn([layer]);
  return { body: [], view: [refEl(kit.name("v"), callback)] };
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function refLayerPlacement() {
  const names = REF_LAYER_CONTEXT_NAMES;
  return { STRICT_READ_UNTRACKED: { misuse: names.filter((name) => REF_LAYER_CONTEXTS[name] === "misuse"), legal: names.filter((name) => REF_LAYER_CONTEXTS[name] === "legal") } };
}
