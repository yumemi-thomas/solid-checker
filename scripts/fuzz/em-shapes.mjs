// The emission shapes (batch 11, the follow-up to the review of #220): the site is the body of a function that no owner enters, reached by a route the owner graph holds no edge for, and the
// host's callback runs it where no owner is current. The owner emission (`owners/owner_emission.rs`, ADR 0868) asks `named_layer_entry_open` whether a layer around an owner-requiring
// operation may be entered by such a route. Three of its stops were taken for proofs, and main certified every program below with no row (Chrome, rc.21, reports NO_OWNER_CLEANUP,
// NO_OWNER_EFFECT and NO_OWNER_CONTEXT at the site):
//
//   em-view-<host>          the walk stopped at a function the census proves a component (it returns JSX and no call enters it, one a reference also enters included) before it looked
//                           at its references: the component-render premise the ruling for the owner-call universe excludes. A function that returns JSX, no tag renders it, held by a
//                           variable (`const pk = h`) that the host's callback calls (`pk!()`)
//   em-gen-<host>           the caller walk took a generator's body for a placed run (N1, the check of B1 on #220): a helper written beside a generator and called from its body
//                           (`function* g() { h(); yield 1; }`, `const it = g()` in the component body), which the host steps with a `for...of`
//   em-self-<host>          the stop at an owner-creating layer returned before the function's references (N2): the callback of `createRoot` is a named function expression that
//                           saves itself (`createRoot(function f() { if (!sv) { sv = f; return; } <site> })`), which the host calls (`sv!()`)
//   <route>-<host>-never    legal    the same declarations, the host runs nothing: the function is held (the generator not stepped, the saved function not called) and never run, so no site runs
//   hosts   handler (a button's click handler), timer (a `setTimeout` callback): where no owner is current; apply (an effect's apply: its stack has none; NO_OWNER_CLEANUP and
//           NO_OWNER_CONTEXT, since a creation there is PRIMITIVE_IN_EFFECT_CALLBACK, a code of its own)
//
// A function that returns no JSX and is entered the same way was already an obligation (`entry-by-reference`), so each stop is the whole of its hole. Recall contexts, the direct
// binding bare (`emShape`), not crossed by the census, out of api-fire's plan.
import { button, contain, decl, domExpr, expr, fn, jsx, num, prim, ret } from "./model.mjs";

export const EM_ROUTES = ["view", "gen", "self"];
export const EM_HOSTS = ["handler", "timer", "apply"];
/** The hosts each code is judged in: the hosts where the runtime reports the code for a call with no owner current. */
export const EM_CODES = {
  NO_OWNER_CLEANUP: ["handler", "timer", "apply"],
  NO_OWNER_EFFECT: ["handler", "timer"],
  NO_OWNER_CONTEXT: ["handler", "timer", "apply"],
};
/** `{ [context]: "misuse" | "legal" }`. */
export const EM_CONTEXTS = Object.fromEntries(EM_ROUTES.flatMap((route) => EM_HOSTS.flatMap((host) => [[`em-${route}-${host}`, "misuse"], [`em-${route}-${host}-never`, "legal"]])));
export const EM_CONTEXT_NAMES = Object.keys(EM_CONTEXTS);

const NAME = new RegExp(`^em-(${EM_ROUTES.join("|")})-(${EM_HOSTS.join("|")})(-never)?$`);
/** `{ route, host, never }` of an `em-*` context name, or null. */
export const emParts = (context) => {
  const match = NAME.exec(context);
  return match ? { route: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes an `em-*` cell takes: the direct binding, no layer. */
export function emShape(context, shape) {
  return !emParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The placement of `stmts` for an `em-*` context (`{ body, view }`), or undefined for any other. */
export function emContext(context, stmts, kit) {
  const parts = emParts(context);
  if (!parts) return undefined;
  const { route, host, never } = parts;
  // What the host's callback does with the route's holder: runs it, or, in the twin, only keeps its name used.
  const hostOf = (setup, holder, text) => {
    const runs = [expr(domExpr(never ? `void ${holder}` : text))];
    switch (host) {
      case "handler": return { body: setup, view: [button(kit.name("b"), runs)] };
      case "timer": return { body: [...setup, expr(prim("timer", fn(runs)))], view: [] };
      case "apply": return { body: [...setup, expr(prim("effect", fn([ret(num(1))]), { apply: fn(runs) }))], view: [] };
      default: throw new Error(`no emission host ${host}`);
    }
  };
  switch (route) {
    case "view": {
      // A function that returns JSX and that no tag renders, held by a variable: its entry is a call through the variable.
      const h = kit.name("h");
      const pk = kit.name("pk");
      const held = decl(h, fn([...stmts, ret(jsx())], { form: "function" }));
      return hostOf([held, expr(domExpr(`const ${pk}: (() => unknown) | undefined = ${h}`))], pk, `${pk}!()`);
    }
    case "gen": {
      // N1: a helper beside a generator, called from its body; the iterator is made in the component's body (the body of a generator does not run yet) and the host steps it.
      const h = kit.name("hh");
      const g = kit.name("g");
      const it = kit.name("it");
      return hostOf([decl(h, fn(stmts, { form: "function" })), expr(domExpr(`function* ${g}() { ${h}(); yield 1; }`)), expr(domExpr(`const ${it} = ${g}()`))], it, `for (const _ of ${it}) { void _; }`);
    }
    case "self": {
      // N2: the root runs the callback at once (it saves itself and returns); the host calls the saved function, which runs on its own stack with no owner.
      const f = kit.name("f");
      const sv = kit.name("sv");
      const guard = expr(domExpr(`if (!${sv}) { ${sv} = ${f}; return; }`));
      const frame = contain("frame", 9, [guard, ...stmts], { head: `createRoot(function ${f}() `, close: ");", imports: ["createRoot"] });
      return hostOf([expr(domExpr(`let ${sv}: (() => void) | undefined`)), frame], sv, `${sv}!()`);
    }
    default: throw new Error(`no emission route ${route}`);
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function emPlacement() {
  return Object.fromEntries(Object.entries(EM_CODES).map(([code, hosts]) => {
    const contexts = EM_CONTEXT_NAMES.filter((name) => hosts.includes(emParts(name).host));
    return [code, { misuse: contexts.filter((name) => EM_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => EM_CONTEXTS[name] === "legal") }];
  }));
}
