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

/**
 * ADR 0713, SC4002: a read of a context that is provided somewhere, but not above this read: a provider in a sibling subtree, and a
 * component rendered once under the provider and once outside. Chrome (rc.21): "Context has no provider above this read". The build
 * before certified both (its census asked only whether a provider exists).
 */
const OPEN_CONTEXTS = {
  "flw-ctx-sibling": (stmts, kit) => ({ body: [decl(kit.name("p"), domExpr("<Ctx value={1}><i /></Ctx>")), ...stmts], view: [] }),
  "flw-ctx-two-renders": (stmts, kit) => {
    const K = kit.name("K");
    return {
      body: [decl(K, fn([...stmts, ret(domExpr("<i />"))])), decl(kit.name("p"), domExpr(`<><Ctx value={1}><${K} /></Ctx><${K} /></>`))],
      view: [],
    };
  },
};

/**
 * ADR 0714, SC4001: the site's function called once in the component (owned) and once in a module-scope branch nothing takes. Chrome
 * (rc.21): quiet, the only run is owned. The build before reported it proven unowned (the module's edge carried the proof whether or
 * not its call runs).
 */
const MIXED_OWNERS = {
  "flw-mixed-dead": (stmts, kit) => {
    const h = kit.name("h");
    return {
      // Raw text: the validator reads names declared in the component, and `h` is not one.
      hoisted: [decl(h, fn(stmts)), expr(domExpr(`if (Math.random() > 2) ${h}();`))],
      body: [expr(domExpr(`${h}()`))],
      view: [],
    };
  },
};

/**
 * ADR 0715, SC2002: the site in a module-scope helper only an object getter calls, the getter read in the component body: no call edge reaches the
 * helper, and the body's owner runs it (Chrome, rc.21: ACTION_CALLED_IN_OWNED_SCOPE). The build before certified it.
 */
const UNREACHED_WRITES = {
  "flw-getter-helper": (stmts, kit) => {
    const h = kit.name("getterRun");
    const o = kit.name("getterHost");
    return {
      // At module scope with the action (`hoisted`), as measured: a helper in the component already had a row. Raw text: the
      // model writes no getter, and the validator reads names declared in the component.
      hoisted: [decl(h, fn(stmts)), decl(o, domExpr(`{ get v() { ${h}(); return 1; } }`))],
      body: [expr(domExpr(`${o}.v`))],
      view: [],
    };
  },
};

export const FLIP_WRITE_CONTEXTS = {
  ACTION_CALLED_IN_OWNED_SCOPE: Object.keys(UNREACHED_WRITES),
  NO_OWNER_CLEANUP: Object.keys(MIXED_OWNERS),
  NO_OWNER_EFFECT: Object.keys(MIXED_OWNERS),
  CONTEXT_NOT_FOUND: Object.keys(OPEN_CONTEXTS),
  PRIMITIVE_IN_FORBIDDEN_SCOPE: Object.keys(DISPATCH_FIRES),
  FLUSH_IN_ACTION: Object.keys(UNPLACED_STEPS),
  FLUSH_IN_EFFECT_CALLBACK: Object.keys(UNPLACED_APPLIES),
  // ADR 0710: the same layer for a computation created there (SC6002).
  PRIMITIVE_IN_EFFECT_CALLBACK: Object.keys(UNPLACED_APPLIES),
};

export function flipWriteContext(context, stmts, kit) {
  const fire = DISPATCH_FIRES[context];
  if (fire) return { body: fire(stmts, kit), view: [] };
  return (UNPLACED_STEPS[context] ?? UNPLACED_APPLIES[context] ?? OPEN_CONTEXTS[context] ?? MIXED_OWNERS[context] ?? UNREACHED_WRITES[context])?.(stmts, kit);
}
