// The flip's generator-step shapes (ADR 0851): a named generator holding the site, stepped somewhere else than where it is called.
// Calling a generator runs none of its body; each step runs the next segment on the stack of the call that makes it. These contexts
// place the site past the generator's first `yield` (or before it, `gstep-unstepped-handler`), with the steps in a handler, a timer, a
// kept or returned iterator, a consuming construct (spread, `Array.from`, destructuring, `yield*`) or a generator method. The census
// crosses every code with them; MISSING_EFFECT_FN lists them because its site is a misuse wherever it runs.
import { button, ctl, decl, domExpr, expr } from "./model.mjs";

export const FLIP_GEN_CONTEXTS = {
  MISSING_EFFECT_FN: ["gstep-handler", "gstep-timer", "gstep-kept", "gstep-returned", "gstep-spread", "gstep-from", "gstep-destructure", "gstep-delegate",
    "gstep-method", "gstep-unstepped-handler"],
};

const gen = (name, stmts, extra = {}) => ({ ...ctl("gstep", stmts), name, ...extra });

/** The placement of `stmts` for a `gstep-*` context, or undefined for any other context. */
export function flipGenContext(context, stmts, kit) {
  if (!context.startsWith("gstep-")) return undefined;
  const g = kit.name("g");
  const it = kit.name("it");
  const b = kit.name("b");
  // The first step runs the segment before the `yield` in the body; the handler's step runs the site, with no owner.
  const first = [gen(g, stmts), decl(it, domExpr(`${g}()`)), expr(domExpr(`${it}.next()`))];
  switch (context) {
    case "gstep-handler": return { body: first, view: [button(b, [expr(domExpr(`${it}.next()`))])] };
    case "gstep-timer": return { body: [...first, expr(domExpr(`setTimeout(() => { ${it}.next(); }, 0)`))], view: [] };
    case "gstep-kept": {
      const k = kit.name("k");
      return { body: [gen(g, stmts), decl(k, domExpr(`{ it: ${g}() }`)), expr(domExpr(`${k}.it.next()`))], view: [button(b, [expr(domExpr(`${k}.it.next()`))])] };
    }
    case "gstep-returned": {
      const mk = kit.name("mk");
      return { body: [gen(g, stmts), decl(mk, domExpr(`() => ${g}()`)), decl(it, domExpr(`${mk}()`)), expr(domExpr(`${it}.next()`))], view: [button(b, [expr(domExpr(`${it}.next()`))])] };
    }
    // A consuming construct steps the generator to the end on the body's stack.
    case "gstep-spread": return { body: [gen(g, stmts), expr(domExpr(`void [...${g}()]`))], view: [] };
    case "gstep-from": return { body: [gen(g, stmts), expr(domExpr(`void Array.from(${g}())`))], view: [] };
    case "gstep-destructure": return { body: [gen(g, stmts), expr(domExpr(`const [, ${kit.name("d")}] = ${g}()`))], view: [] };
    case "gstep-delegate": {
      const outer = kit.name("o");
      return { body: [gen(g, stmts), expr(domExpr(`function* ${outer}() { yield* ${g}(); }`)), expr(domExpr(`void [...${outer}()]`))], view: [] };
    }
    case "gstep-method": {
      const holder = kit.name("h");
      return {
        body: [gen("g", stmts, { holder }), decl(it, domExpr(`${holder}.g()`)), expr(domExpr(`${it}.next()`))],
        view: [button(b, [expr(domExpr(`${it}.next()`))])],
      };
    }
    // The other direction: the generator is called in a handler, and its only step runs under a root there.
    case "gstep-unstepped-handler":
      return {
        body: [gen(g, stmts, { mode: "first" })],
        view: [button(b, [expr(domExpr(`const ${it} = ${g}(); createRoot(() => { ${it}.next(); })`, undefined, ["createRoot"]))])],
      };
    default: return undefined;
  }
}
