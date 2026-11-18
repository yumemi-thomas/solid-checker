// The promise-unconsumed shapes (ADR 1090 G1, the Fable review of be4990c64): the site, a read after an await, is in an async function whose every reference written in code
// looks like a call nothing consumes, and an async memo still awaits the function's flight, so rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read. be4990c64 ended both sites
// with `AfterAwaitSafe::PromiseUnconsumed` and certified the programs (a false clean):
//
//   pu-then-sub    F1: the handler of `Kp.resolve().then(async () => { await ..; <site> })` on a `Promise` subclass whose constructor keeps `this`. The derived promise is a `Kp`, the
//                  memo awaits the kept instance, so the handler's flight is the memo's. be4990c64 took any receiver whose callee the default library declares for a native promise
//   pu-loop        F2: `for (let i = 0; i < 2; i++) { <site>; await .. }` in a named async function called only by discarded calls. The loop's first pass reads before the first
//                  await, on the caller's stack: the caller is a `toString` method the memo's template literal runs after its own await, so the read is the memo's untracked read
//
// The third finding (F3, a direct `eval` calling the function where its promise is kept) cannot be generated: the code's variants read a signal tuple or a store, and a program
// with direct `eval` withdraws their slots (SC9012 `tuple-iteration-written` on the site's line, a withholding that covers the code). Fixture after-await-promise-unconsumed-eval
// holds it with a memo accessor (rediscovery.json). Recall contexts, the direct binding bare (`puShape`), one program per cell, not crossed by the census, out of api-fire's plan.
import { awaitStmt, call, ctl, decl, domExpr, expr, fn, num, prim, ref, ret } from "./model.mjs";

export const PU_CONTEXTS = { "pu-then-sub": "misuse", "pu-loop": "misuse" };
export const PU_CONTEXT_NAMES = Object.keys(PU_CONTEXTS);
export const PU_CODES = ["UNTRACKED_READ_AFTER_AWAIT"];

/** The base shapes a `pu-*` cell takes: the direct binding, no layer. */
export function puShape(context, shape) {
  return !PU_CONTEXT_NAMES.includes(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** An async memo whose compute runs `stmts`, and the effect that reads it (as `memo-async` does: the memo computes when read). */
function memoOf(kit, stmts) {
  const m = kit.name("m");
  return [decl(m, prim("memo", fn([...stmts, ret(num(1))], { async: true }))), expr(prim("effect", fn([ret(call(ref(m)))]), { apply: fn([]) }))];
}

/**
 * The placement of `stmts` (the code's own site) for a `pu-*` context (`{ hoisted, body, view }`), or undefined for any other. The function and its caller are written at module
 * scope with the component's signals (`hoisted`): be4990c64's proof refused every function written in a component, so only the module-scope spelling was certified. The memo that
 * consumes the flight is the component's.
 */
export function puContext(context, stmts, kit) {
  if (!PU_CONTEXT_NAMES.includes(context)) return undefined;
  switch (context) {
    case "pu-then-sub": {
      const kp = kit.name("Kp");
      const subclass = expr(domExpr(`class ${kp} extends Promise<void> { static last?: Promise<unknown>; constructor(executor: (resolve: (value: void) => void, reject: (reason?: unknown) => void) => void) { super(executor); ${kp}.last = this; } }`));
      // The handler reads after a timer, so the memo (made when the component renders, after module evaluation) is already awaiting the kept promise.
      const later = expr(domExpr("await new Promise((resolve) => setTimeout(resolve, 0))"));
      const handler = expr(prim("then", fn([later, ...stmts], { async: true }), { receiver: `${kp}.resolve()` }));
      return { hoisted: [subclass, handler], body: memoOf(kit, [expr(domExpr(`await ${kp}.last`))]), view: [] };
    }
    case "pu-loop": {
      const f = kit.name("pf");
      const o = kit.name("po");
      const held = decl(f, fn([ctl("loop-await-pre", stmts)], { async: true }));
      const caller = expr(domExpr(`const ${o} = { toString() { ${f}(); return ""; } }`));
      return { hoisted: [held, caller], body: memoOf(kit, [awaitStmt(), expr(domExpr("void `${" + o + "}`"))]), view: [] };
    }
    default: throw new Error(`no promise-unconsumed context ${context}`);
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function puPlacement() {
  return Object.fromEntries(PU_CODES.map((code) => [code, { misuse: PU_CONTEXT_NAMES.filter((context) => PU_CONTEXTS[context] === "misuse"), legal: [] }]));
}
