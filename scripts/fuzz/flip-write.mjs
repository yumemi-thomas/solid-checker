// The flip's write track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_WRITE_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipWriteContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
import { button, call, decl, domExpr, expr, fn, listenExpr, member, num, obj, prim, ref, ret } from "./model.mjs";

/**
 * ADR 0705, the DOM dispatch list: a member that dispatches on its caller's stack, fired from `onSettled` (a leaf owner), with a
 * listener for the event it delivers holding the site. Measured in Chrome (rc.21): each throws PRIMITIVE_IN_FORBIDDEN_SCOPE in the
 * listener; the build before certified both. `form-submit` targets an iframe so the harness page does not navigate; `report-error`
 * cancels the reported error. Not here: the blur family (`replaceWith`, `replaceChild`, `replaceChildren`) needs a focus, and a
 * `focus()` call is itself a hazard that withholds the program on every build (ADR 0645); `History.go` and `Location.*` end the page.
 */
const DISPATCH_FIRES = {
  "flw-settled-form-submit": (stmts, kit) => {
    const el = kit.name("el");
    const frame = kit.name("fr");
    return [
      decl(el, domExpr('document.createElement("form")')),
      expr(domExpr(`document.body.append(${el})`)),
      decl(frame, domExpr('document.createElement("iframe")')),
      expr(domExpr(`${frame}.name = "sink"`)),
      expr(domExpr(`document.body.append(${frame})`)),
      expr(domExpr(`${el}.target = "sink"`)),
      expr(listenExpr(el, "formdata", fn(stmts))),
      expr(prim("settled", fn([expr(domExpr(`${el}.submit()`))]))),
    ];
  },
  "flw-settled-report-error": (stmts) => [
    expr(domExpr('window.addEventListener("error", (event) => event.preventDefault())')),
    expr(listenExpr("window", "error", fn(stmts))),
    expr(prim("settled", fn([expr(domExpr('reportError(new Error("x"))'))]))),
  ],
};

/**
 * ADR 0708, SC2006: the site in a function no call edge places (an object slot), called from an action's body, so a step is
 * on the stack when it runs (Chrome, rc.21: FLUSH_IN_ACTION). The build before certified it.
 */
const UNPLACED_STEPS = {
  "flw-action-slot": (stmts, kit) => {
    const holder = kit.name("h");
    const a = kit.name("a");
    return {
      body: [decl(holder, obj([["go", fn(stmts)]])), decl(a, prim("action", fn([expr(call(member(ref(holder), "go")))])))],
      view: [button(kit.name("b"), [expr(call(ref(a)))])],
    };
  },
};

/**
 * ADR 0709, SC2008: the site in an object slot an effect's apply calls, so it runs inside the flush draining effects (Chrome,
 * rc.21: FLUSH_IN_EFFECT_CALLBACK). The build before certified it.
 */
const UNPLACED_APPLIES = {
  // The holder at module scope: a slot written in the component was already an obligation (ADR 0516).
  "flw-apply-slot-module": (stmts, kit) => {
    const holder = kit.name("h");
    return {
      hoisted: [decl(holder, obj([["go", fn(stmts)]]))],
      // Raw text: the validator reads names declared in the component, and the holder is not one.
      body: [expr(prim("effect", fn([ret(num(1))]), { apply: fn([expr(domExpr(`${holder}.go()`))]) }))],
      view: [],
    };
  },
};

export const FLIP_WRITE_CONTEXTS = {
  PRIMITIVE_IN_FORBIDDEN_SCOPE: Object.keys(DISPATCH_FIRES),
  FLUSH_IN_ACTION: Object.keys(UNPLACED_STEPS),
  FLUSH_IN_EFFECT_CALLBACK: Object.keys(UNPLACED_APPLIES),
};

export function flipWriteContext(context, stmts, kit) {
  const fire = DISPATCH_FIRES[context];
  if (fire) return { body: fire(stmts, kit), view: [] };
  return (UNPLACED_STEPS[context] ?? UNPLACED_APPLIES[context])?.(stmts, kit);
}
