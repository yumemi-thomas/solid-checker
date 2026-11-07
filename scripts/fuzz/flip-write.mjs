// The flip's write track (scripts/flip/PLAN.md): recall contexts per runtime code, and the program of each. Empty at the base: a rule's
// commit adds its shapes here, so neither track edits misuse.mjs or the other's file.
//   FLIP_WRITE_CONTEXTS  { [code]: [context, ...] }, appended to that code's recallContexts
//   flipWriteContext(context, stmts, kit)  the program, as a misuse.mjs context returns one, or undefined when the context is not this file's
import { button, call, ctl, decl, domExpr, expr, fn, iife, listenExpr, member, num, obj, prim, ref, ret } from "./model.mjs";

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
 * ADRs 0715 (SC2002), 0721 (SC2001): the site in a module-scope helper only an object getter calls, the getter read in the component body, or
 * in a module object's method: no call edge reaches it, and the body's owner runs it (Chrome, rc.21: ACTION_CALLED_IN_OWNED_SCOPE,
 * REACTIVE_WRITE_IN_OWNED_SCOPE). The build before certified both.
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
  // ADR 0721: the site directly in a method of a module-scope object the component body calls (raw text: the validator refuses a
  // component reference to a hoisted name).
  "flw-module-method": (stmts, kit) => {
    const o = kit.name("methodHost");
    return { hoisted: [decl(o, obj([["go", fn(stmts, { form: "method" })]]))], body: [expr(domExpr(`${o}.go()`))], view: [] };
  },
};

/**
 * ADR 0866, every owner rule: the site under `if (getOwner())` in an async function the component body calls. A suspension in the guarded
 * block leaves the stack the probe saw (Chrome, rc.21: the site's NO_OWNER_* code), so the guard is no proof; the build before dropped
 * the requirement. `flw-guard-early` and `flw-guard-probe-loop` are their legal twins.
 */
const guardedAsync = (form) => (stmts) => ({ body: [expr(iife(fn([ctl(form, stmts)], { async: true })))], view: [] });
const GUARDED = { "flw-guard-for-await": guardedAsync("guard-for-await"), "flw-guard-loop": guardedAsync("guard-loop"), "flw-guard-await": guardedAsync("guard-await"), "flw-guard-yield": (stmts) => ({ body: [ctl("guard-yield", stmts)], view: [] }) };
const GUARDED_LEGAL = { "flw-guard-early": guardedAsync("guard-early"), "flw-guard-probe-loop": guardedAsync("guard-probe-loop") };
/**
 * F2 (scout's owner-context map): the site's function is called in the component body, so an owned edge reaches it, and it is also handed
 * somewhere a later click runs it with no owner. Chrome (rc.21): the site's NO_OWNER_* code on the click. The build before read the owned
 * edge as every entry. `flw-owned-twice` and `flw-owned-removed` are legal: every run is on the body's stack.
 */
const ownedAnd = (place, { jsx: returnsJsx = false } = {}) => (stmts, kit) => {
  const f = kit.name("f");
  const { before = [], after, fires } = place(f, kit);
  const body = returnsJsx ? [...stmts, ret(domExpr("<li />"))] : stmts;
  return { body: [decl(f, fn(body)), expr(call(ref(f))), ...before, ...(after ?? [])], view: [button(kit.name("b"), [expr(domExpr(fires))])] };
};
const ESCAPES = {
  "flw-owned-assigned": ownedAnd((f, kit) => {
    const el = kit.name("el");
    return { before: [decl(el, domExpr('document.createElement("button")')), expr(domExpr(`${el}.onclick = ${f}`))], fires: `${el}.click()` };
  }),
  "flw-owned-stored": ownedAnd((f, kit) => {
    const items = kit.name("items");
    return { before: [decl(items, domExpr("[] as (() => void)[]")), expr(domExpr(`${items}.push(${f})`))], fires: `${items}[0]()` };
  }),
  "flw-owned-member": ownedAnd((f, kit) => {
    const slot = kit.name("slot");
    return { before: [decl(slot, domExpr("{ run: () => {} }")), expr(domExpr(`${slot}.run = ${f}`))], fires: `${slot}.run()` };
  }),
  "flw-owned-map": ownedAnd((f, kit) => {
    const map = kit.name("map");
    return { before: [decl(map, domExpr("new Map<string, () => void>()")), expr(domExpr(`${map}.set("k", ${f})`))], fires: `${map}.get("k")!()` };
  }),
  "flw-owned-lib": ownedAnd((f, kit) => {
    const [reg, on] = [kit.name("reg"), kit.name("on")];
    return { before: [decl(reg, domExpr("[] as (() => void)[]")), decl(on, domExpr(`(cb: () => void) => { ${reg}.push(cb); }`)), expr(domExpr(`${on}(${f})`))], fires: `${reg}[0]()` };
  }),
  // Reviewer D13 on ADR 0868: each premise of the first cut, broken.
  // 1: a Value reference beside a run the census must see.
  "flw-owned-value-foreach": ownedAnd((f) => ({ before: [expr(domExpr(`document.body.removeEventListener("click", ${f})`))], fires: `[1].forEach(${f})` })),
  // 2: a value-only argument that keeps the function (`this`, an `object` parameter).
  "flw-owned-keep-this": ownedAnd((f, kit) => {
    const [reg, keep] = [kit.name("reg"), kit.name("keep")];
    return { before: [decl(reg, domExpr("[] as (() => void)[]")), decl(keep, domExpr(`function (this: () => void) { ${reg}.push(this); }`)), expr(domExpr(`${keep}.call(${f})`))], fires: `${reg}[0]()` };
  }),
  "flw-owned-keep-object": ownedAnd((f, kit) => {
    const [reg, keep] = [kit.name("reg"), kit.name("keep")];
    return { before: [decl(reg, domExpr("[] as object[]")), decl(keep, domExpr(`(o: object) => { ${reg}.push(o); }`)), expr(domExpr(`${keep}(${f})`))], fires: `(${reg}[0] as () => void)()` };
  }),
  // 3: an EventTarget whose removeEventListener keeps the listener.
  "flw-owned-remove-impostor": ownedAnd((f, kit) => {
    const [reg, t] = [kit.name("reg"), kit.name("t")];
    return { before: [decl(reg, domExpr("[] as (() => void)[]")), decl(t, domExpr(`new (class extends EventTarget { override removeEventListener(_t: string, cb: any) { ${reg}.push(cb); } })() as EventTarget`)), expr(domExpr(`${t}.removeEventListener("x", ${f})`))], fires: `${reg}[0]()` };
  }),
  // 4: an unnamed layer held by an object slot, called in the body and assigned to a handler.
  "flw-owned-slot": (stmts, kit) => {
    const [o, el] = [kit.name("o"), kit.name("el")];
    return { body: [decl(o, obj([["run", fn(stmts)]])), expr(call(member(ref(o), "run"))), decl(el, domExpr('document.createElement("button")')), expr(domExpr(`${el}.onclick = ${o}.run`))], view: [button(kit.name("b"), [expr(domExpr(`${el}.click()`))])] };
  },
  // 5: a layer that returns JSX, so it may be a component, never rendered as one.
  "flw-owned-jsx-layer": ownedAnd((f, kit) => {
    const el = kit.name("el");
    return { before: [decl(el, domExpr('document.createElement("button")')), expr(domExpr(`${el}.onclick = () => { ${f}(); }`))], fires: `${el}.click()` };
  }, { jsx: true }),
  // 6: `eval` reaches the binding by name.
  "flw-owned-eval": ownedAnd((f, kit) => {
    const g = kit.name("g");
    return { before: [decl(g, domExpr(`eval("${f}") as () => void`))], fires: `${g}()` };
  }),
  // R29: a primitive's whole argument is placed by the graph only where the primitive runs it under a known owner. `action` keeps a generator
  // and steps it on the click; `createContext` holds a value (Chrome, rc.21: the site's NO_OWNER_* code on the click).
  "flw-owned-action": ownedAnd((f, kit) => {
    const act = kit.name("act");
    return { before: [decl(act, domExpr(`action(${f} as any)`, undefined, ["action"]))], fires: `${act}()` };
  }),
  "flw-owned-action-gen": (stmts, kit) => {
    const [f, act] = [kit.name("f"), kit.name("act")];
    return { body: [decl(f, fn(stmts, { form: "generator" })), expr(domExpr(`${f}().next()`)), decl(act, domExpr(`action(${f})`, undefined, ["action"]))], view: [button(kit.name("b"), [expr(domExpr(`${act}()`))])] };
  },
  "flw-owned-context": ownedAnd((f, kit) => {
    const [C, g] = [kit.name("C"), kit.name("g")];
    return { before: [decl(C, domExpr(`createContext(${f})`, undefined, ["createContext"])), decl(g, domExpr(`useContext(${C})`, undefined, ["useContext"]))], fires: `${g}()` };
  }),
  "flw-owned-returned": (stmts, kit) => {
    const [f, u, g] = [kit.name("f"), kit.name("u"), kit.name("g")];
    return { body: [decl(u, fn([decl(f, fn(stmts)), expr(call(ref(f))), ret(ref(f))])), decl(g, call(ref(u)))], view: [button(kit.name("b"), [expr(call(ref(g)))])] };
  },
};
const ESCAPES_LEGAL = {
  "flw-owned-twice": (stmts, kit) => {
    const f = kit.name("f");
    return { body: [decl(f, fn(stmts)), expr(call(ref(f))), expr(call(ref(f)))], view: [] };
  },
  "flw-owned-slot-called": (stmts, kit) => {
    const o = kit.name("o");
    return { body: [decl(o, obj([["run", fn(stmts)]])), expr(call(member(ref(o), "run"))), expr(call(member(ref(o), "run")))], view: [] };
  },
  "flw-owned-removed": (stmts, kit) => {
    const f = kit.name("f");
    return { body: [decl(f, fn(stmts)), expr(call(ref(f))), expr(domExpr(`document.body.removeEventListener("click", ${f})`))], view: [] };
  },
};
const OWNER_CODES = ["NO_OWNER_CLEANUP", "NO_OWNER_EFFECT", "NO_OWNER_BOUNDARY", "NO_OWNER_CONTEXT", "SETTLED_CLEANUP_UNOWNED"];
/** Legal recall contexts per runtime code, appended to that code's recallLegal. */
export const FLIP_WRITE_LEGAL = Object.fromEntries(OWNER_CODES.map((code) => [code, [...Object.keys(GUARDED_LEGAL), ...Object.keys(ESCAPES_LEGAL)]]));

export const FLIP_WRITE_CONTEXTS = {
  ACTION_CALLED_IN_OWNED_SCOPE: Object.keys(UNREACHED_WRITES),
  // ADR 0721: the same layers for a setter or refresh (SC2001).
  REACTIVE_WRITE_IN_OWNED_SCOPE: Object.keys(UNREACHED_WRITES),
  NO_OWNER_CLEANUP: Object.keys(MIXED_OWNERS),
  NO_OWNER_EFFECT: Object.keys(MIXED_OWNERS),
  CONTEXT_NOT_FOUND: Object.keys(OPEN_CONTEXTS),
  PRIMITIVE_IN_FORBIDDEN_SCOPE: Object.keys(DISPATCH_FIRES),
  FLUSH_IN_ACTION: Object.keys(UNPLACED_STEPS),
  FLUSH_IN_EFFECT_CALLBACK: Object.keys(UNPLACED_APPLIES),
  // ADR 0710: the same layer for a computation created there (SC6002).
  PRIMITIVE_IN_EFFECT_CALLBACK: Object.keys(UNPLACED_APPLIES),
};

for (const code of OWNER_CODES) FLIP_WRITE_CONTEXTS[code] = [...(FLIP_WRITE_CONTEXTS[code] ?? []), ...Object.keys(GUARDED), ...Object.keys(ESCAPES)];

export function flipWriteContext(context, stmts, kit) {
  const fire = DISPATCH_FIRES[context];
  if (fire) return { body: fire(stmts, kit), view: [] };
  return (GUARDED[context] ?? GUARDED_LEGAL[context] ?? ESCAPES[context] ?? ESCAPES_LEGAL[context] ?? UNPLACED_STEPS[context] ?? UNPLACED_APPLIES[context] ?? OPEN_CONTEXTS[context] ?? MIXED_OWNERS[context] ?? UNREACHED_WRITES[context])?.(stmts, kit);
}
