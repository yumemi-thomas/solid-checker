// The flip's write track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_WRITE_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipWriteContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
import { decl, domExpr, expr, fn, listenExpr, prim } from "./model.mjs";

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

export const FLIP_WRITE_CONTEXTS = {
  PRIMITIVE_IN_FORBIDDEN_SCOPE: Object.keys(DISPATCH_FIRES),
};

export function flipWriteContext(context, stmts, kit) {
  const fire = DISPATCH_FIRES[context];
  return fire ? { body: fire(stmts, kit), view: [] } : undefined;
}
