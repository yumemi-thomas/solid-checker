// The promise-unconsumed shapes (ADR 1090 G1, the Fable review of be4990c64): the site, a read after an await, is in an async function whose every reference written in code
// looks like a call nothing consumes, and an async memo still awaits the function's flight, so rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read. be4990c64 ended both sites
// with `AfterAwaitSafe::PromiseUnconsumed` and certified the programs (a false clean):
//
//   pu-then-sub    F1: the handler of `Kp.resolve().then(async () => { await ..; <site> })` on a `Promise` subclass whose constructor keeps `this`. The derived promise is a `Kp`, the
//                  memo awaits the kept instance, so the handler's flight is the memo's. be4990c64 took any receiver whose callee the default library declares for a native promise
//   pu-loop        F2: `for (let i = 0; i < 2; i++) { <site>; await .. }` in a named async function called only by discarded calls. The loop's first pass reads before the first
//                  await, on the caller's stack: the caller is a `toString` method the memo's template literal runs after its own await, so the read is the memo's untracked read
//   pu-class       G1 of the second review: `pb.K = class { x = ph(); }` after the function's await, `ph` a helper whose body is the site. A field
//                  initializer is no function fact, so the call was the function's read after its await, but it runs where `new` runs: a `toString`
//                  the memo's template literal runs (`new pb.K!()`). A read written in the initializer itself was already refused
//                  (`field-initializer-unplaced`), so the helper is the hole
//
// The third finding (F3, a direct `eval` calling the function where its promise is kept) cannot be generated: the code's variants read a signal tuple or a store, and a program
// with direct `eval` withdraws their slots (SC9012 `tuple-iteration-written` on the site's line, a withholding that covers the code). Fixture after-await-promise-unconsumed-eval
// holds it with a memo accessor (rediscovery.json). Recall contexts, the direct binding bare (`puShape`), one program per cell, not crossed by the census, out of api-fire's plan.
import { awaitStmt, call, ctl, decl, domExpr, expr, fn, num, obj, prim, ref, ret } from "./model.mjs";

export const PU_CONTEXTS = { "pu-then-sub": "misuse", "pu-loop": "misuse", "pu-class": "misuse", "pu-coerce-direct": "misuse", "pu-coerce-wrapper": "misuse", "pu-coerce-iife": "misuse", "pu-in-direct": "misuse", "pu-in-wrapper": "misuse", "pu-in-iife": "misuse", "pu-instanceof-direct": "misuse", "pu-instanceof-wrapper": "misuse", "pu-instanceof-iife": "misuse", "pu-key-direct": "misuse", "pu-key-wrapper": "misuse", "pu-key-iife": "misuse" };
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
    case "pu-class": {
      // The second review of 1090 G1 (G1): the site is a class's instance field initializer, written after the function's await, which no
      // function fact holds, so the read was the function's; it runs where `new` runs: a `toString` the memo's template literal runs.
      const box = kit.name("pb");
      const helper = kit.name("ph");
      const f = kit.name("pf");
      const o = kit.name("po");
      const field = expr(domExpr(`${box}.K = class { x = ${helper}(); }`));
      return {
        hoisted: [decl(helper, fn(stmts)), expr(domExpr(`const ${box}: { K?: new () => unknown } = {}`)), decl(f, fn([awaitStmt(), field], { async: true })), expr(domExpr(`${f}()`)), expr(domExpr(`const ${o} = { toString() { new ${box}.K!(); return ""; } }`))],
        body: memoOf(kit, [awaitStmt(), expr(domExpr("void `${" + o + "}`"))]),
        view: [],
      };
    }
    default: {
      // ADR 1097, the reviews of its widening: an async memo's continuation calls a setter whose argument runs project code with no call, which
      // holds the site: a coercion (`${po}`), `in` (a Proxy's `has` trap), `instanceof` (`Symbol.hasInstance`) or a computed key (its value's
      // `toString`). `direct`: the call in the continuation (ADR 0976's `LibraryRunsNothing`); `wrapper`: a `const` wrapper of it; `iife`: an IIFE.
      const parts = /^pu-(coerce|in|instanceof|key)-(direct|wrapper|iife)$/.exec(context);
      if (!parts) throw new Error(`no promise-unconsumed context ${context}`);
      const [, probe, form] = parts;
      const holder = kit.name("po");
      const target = kit.name("pt");
      const set = kit.name("pw");
      const wrap = kit.name("pv");
      // A trap returns what its protocol asks (`has` and `Symbol.hasInstance` a boolean, `toString` anything primitive).
      const trap = fn([...stmts, ret(probe === "in" || probe === "instanceof" ? domExpr("true") : num(1))]);
      const setup = {
        coerce: [decl(holder, obj([["toString", trap]]))],
        key: [decl(holder, obj([["toString", trap]]))],
        in: [decl(holder, obj([["has", trap]])), expr(domExpr(`const ${target} = new Proxy({}, ${holder})`))],
        instanceof: [decl(holder, obj([["[Symbol.hasInstance]", trap]])), expr(domExpr(`const ${target} = {}`))],
      }[probe];
      const argument = { coerce: `\`\${${holder}}\``, key: `{ [${holder} as unknown as string]: 1 }`, in: `"k" in ${target}`, instanceof: `${target} instanceof (${holder} as unknown as Function)` }[probe];
      const write = `${set}(${argument})`;
      const body = [...setup, expr(domExpr(`const [, ${set}] = createSignal<unknown>(0)`, undefined, ["createSignal"]))];
      if (form === "wrapper") body.push(expr(domExpr(`const ${wrap} = () => { ${write}; }`)));
      const call = form === "direct" ? write : form === "wrapper" ? `${wrap}()` : `(() => { ${write}; })()`;
      // In the component's body: an object member there no call names holds the site (at module scope its read had rows of its own on every build).
      return { body: [...body, ...memoOf(kit, [awaitStmt(), expr(domExpr(call))])], view: [] };
    }
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function puPlacement() {
  return Object.fromEntries(PU_CODES.map((code) => [code, { misuse: PU_CONTEXT_NAMES.filter((context) => PU_CONTEXTS[context] === "misuse"), legal: [] }]));
}
