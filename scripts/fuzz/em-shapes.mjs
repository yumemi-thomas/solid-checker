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
//   em-hoc-<host>           the census takes a function handed to a wrapper call for a component (`const Wr = wrap(() => { <site>; return <p />; })`, B1 of the review of #221) and the walk
//                           stopped at it; the wrapper keeps what it is handed (`const wrap = (c) => { sv = c; return c as () => any; }`) and the host calls the kept function (`sv!()`), a tag renders `Wr` too
//   em-eval-<host>          a direct `eval` reaches any binding by name (B3 of the review of #221) and the walk asked for it after the stop that closes a component only tags render: the
//                           component `function K() { <site>; return <p />; }` is rendered by a tag, and the host's callback calls it by name through `eval("K()")`
//   em-ns-<host>            the component stop read symbol references only (B2 of the review of #221): the component is exported by the program's module, a tag renders it, and the host's
//                           callback calls it through the module's own namespace by a key built at run time (`self["K1"]()`, `import * as self from "./App"`), which names no symbol
//   em-nss-<host>           the same with the component published by an export specifier, no export statement that contains the function (R1 of the re-review of #221):
//                           `function K1() { <site>; return <p />; } export { K1 };`, called through `self["K1"]()`
//   em-nsa-<host>           an arrow `const K1 = () => { .. }` published renamed (`export { K1 as Out1 }`, R1), called through `self["Out1"]()`
//   em-nsf-<host>           a `const` bound to a function expression that carries a name of its own, published by a specifier (N1 of the second re-review of #221): `const K1 =
//                           function Own1() { <site>; return <span />; }; export { K1 }`; `em-nsg-<host>` the same spelling, `const K1 = function K1() { .. }`
//   em-nso-<host>           `em-ns` in an OPEN program (R2 and R3: a publishing manifest, `programBoundary: open`): the namespace census is a closed program's and code the
//                           program does not hold may call any export; the caller in the program is the cell's, the unseen importer cannot be written (rediscovery.json)
//   <route>-<host>-never    legal    the same declarations, the host runs nothing: the function is held (the generator not stepped, the saved function not called, the eval evaluating `1`) and never run, so no site runs
//   hosts   handler (a button's click handler), timer (a `setTimeout` callback): where no owner is current; apply (an effect's apply: its stack has none; NO_OWNER_CLEANUP and
//           NO_OWNER_CONTEXT, since a creation there is PRIMITIVE_IN_EFFECT_CALLBACK, a code of its own)
//
// A function that returns no JSX and is entered the same way was already an obligation (`entry-by-reference`), so each stop is the whole of its hole. Recall contexts, the direct
// binding bare (`emShape`), not crossed by the census, out of api-fire's plan.
import { button, contain, decl, domExpr, expr, fn, jsx, mounted, num, prim, ret } from "./model.mjs";

export const EM_ROUTES = ["view", "gen", "self", "hoc", "eval", "ns", "nss", "nsa", "nso", "nsf", "nsg"];
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

/** The base shapes an `em-*` cell takes: the direct binding, no layer. The `ns` routes write the component at module scope, where the values of the shape's setup go with it: a reaction
 * (`reaction-track-owned`) is then a creation with no owner of its own, so that variant is no twin of the others. */
export function emShape(context, shape) {
  const parts = emParts(context);
  if (!parts) return true;
  if (parts.route.startsWith("ns") && shape.variant === "reaction-track-owned") return false;
  return shape.binding === "direct" && (shape.layer ?? "none") === "none";
}

/** The placement of `stmts` for an `em-*` context (`{ body, view }`), or undefined for any other. */
export function emContext(context, stmts, kit) {
  const parts = emParts(context);
  if (!parts) return undefined;
  const { route, host, never } = parts;
  // What the host's callback does with the route's holder: runs it, or, in the twin, only keeps its name used (`twin` says what else it does there). `shown` are view items the route adds.
  const hostOf = (setup, holder, text, { twin = `void ${holder}`, shown = [] } = {}) => {
    const runs = [expr(domExpr(never ? twin : text))];
    switch (host) {
      case "handler": return { body: setup, view: [button(kit.name("b"), runs), ...shown] };
      case "timer": return { body: [...setup, expr(prim("timer", fn(runs)))], view: shown };
      case "apply": return { body: [...setup, expr(prim("effect", fn([ret(num(1))]), { apply: fn(runs) }))], view: shown };
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
    case "hoc": {
      // B1 (the review of #221): the arrow that returns JSX is the argument of a wrapper call that keeps it, `const Wr = wrap(() => { <site>; return <p />; })`, and a tag renders `Wr`; the host
      // calls the kept function, which runs on its own stack with no owner.
      const w = kit.name("wr");
      const sv = kit.name("sv");
      const bound = kit.name("Wr");
      const keep = [expr(domExpr(`let ${sv}: (() => unknown) | undefined`)), expr(domExpr(`const ${w} = (c: () => unknown) => { ${sv} = c; return c as () => any; }`))];
      const held = contain("frame", 9, stmts, { head: `const ${bound} = ${w}(() => `, close: ");", after: ["return <p />;"] });
      return hostOf([...keep, held], sv, `${sv}!()`, { shown: [mounted(kit.name("m"), domExpr(`<${bound} />`))] });
    }
    case "eval": {
      // B3 (the review of #221): the component only a tag renders; the host's callback reaches it by name through direct `eval`, which no reference of the program shows.
      const k = kit.name("K");
      const held = contain("frame", 9, stmts, { head: `function ${k}() `, close: "", after: ["return <p />;"] });
      return hostOf([held], k, `eval("${k}()")`, { twin: `eval("1")`, shown: [mounted(kit.name("m"), domExpr(`<${k} />`))] });
    }
    case "ns": case "nss": case "nsa": case "nso": {
      // B2 (the review of #221): the component the program's module exports and a tag renders; the host's callback calls it through the module's own namespace by a key built at run time
      // (ADR 0934's `export-namespace-call`), so no reference names it. The twin keeps the namespace used and calls nothing. The re-review's three: `nss` publishes it by an export
      // specifier (`export { K1 }`, R1), `nsa` an arrow `const` by a renamed one (`export { K1 as Out1 }`, R1) and `nso` is `ns` in an OPEN program (a publishing manifest, R2: the
      // namespace census is a closed program's).
      const k = kit.name("K");
      const out = route === "nsa" ? kit.name("Out") : k;
      const component = fn([...stmts, ret(jsx())], { form: route === "nsa" ? "arrow" : "function" });
      const hoisted = route === "nss" ? [decl(k, component), expr(domExpr(`export { ${k} }`))]
        : route === "nsa" ? [decl(k, component), expr(domExpr(`export { ${k} as ${out} }`))]
          : [decl(k, component, { exported: true })];
      const run = `self[${JSON.stringify([out.slice(0, 1), out.slice(1)])}.join("") as "${out}"]()`;
      return {
        ...hostOf([], k, run, { twin: "void self", shown: [mounted(kit.name("m"), domExpr(`<${k} />`))] }), hoisted, files: {}, importLines: ['import * as self from "./App";'],
        ...(route === "nso" ? { boundary: "open" } : {}),
      };
    }
    case "nsf": case "nsg": {
      // N1 of the second re-review of #221: a `const` bound to a function expression that carries a name of its own, published by an export specifier. `nsf` gives the expression
      // a name unlike the binding's (`const K1 = function Own1() { .. }`), `nsg` the same spelling (`const K1 = function K1() { .. }`): the specifier resolves to the `const`, and the
      // name inside is visible in the body only. A tag renders `K1`; the host calls it through the module's namespace by a key built at run time.
      const k = kit.name("K");
      const own = route === "nsg" ? k : kit.name("Own");
      const held = contain("frame", 9, stmts, { head: `const ${k} = function ${own}() `, close: ";", after: ["return <span />;"] });
      const run = `self[${JSON.stringify([k.slice(0, 1), k.slice(1)])}.join("") as "${k}"]()`;
      return { ...hostOf([], k, run, { twin: "void self", shown: [mounted(kit.name("m"), domExpr(`<${k} />`))] }), hoisted: [held, expr(domExpr(`export { ${k} }`))], files: {}, importLines: ['import * as self from "./App";'] };
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
