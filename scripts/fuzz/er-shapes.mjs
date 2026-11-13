// The entry-by-reference shapes (batch 11, slice 6, the review of #220, F1-F3): the site is the body of a function the host's callback reaches by a REFERENCE, where the
// owner-call universe's silences (`EffectCreationRunsOffApply`, `EffectCreationOwnerSetAround`, `OwnerCurrent`, `LeafOwnerAbsent`) had trusted the place a function is
// written or a render that enters it, and said nothing of the other route. Chrome (rc.21, the real oracle) reports at each site; the head of #220 ended every one with a
// named silence (certified, no row), and main had no row either:
//
//   er-<route>-<host>          misuse   the host's callback calls the function through the holder: the site runs on the apply's (or the leaf owner's) stack
//   er-<route>-<host>-never    legal    the same declarations and no call: the function is held and never run, so no site runs
//     routes  view    a function that RETURNS JSX, no tag renders it, held by a variable (`const pk = h`): the graph gave it an owned context and the emission's test stopped at a
//                     function the census proves a component, before it looked at its references (F1: the component-render premise the ruling excludes)
//             array   a named function registered as an element's handler (`<button onClick={h}>`) that an array also holds, called by the host as `all[0]!()` (F2)
//             saved   a named handler registered the same way and also assigned to a variable the host calls (F2: a handler saved and called from an apply)
//             timer   a named function handed to `setTimeout` and also assigned to a variable the host calls (F2: the timer-callback form)
//             generator  a generator literal `untrack` hands back, `const it = untrack(function* () { <site>; yield 1; })`, that the host steps with `it.next()`: its body runs where the
//                     apply steps the iterator, and a generator's body is no plain site (F2)
//             root    the callback of `createRoot` is a GENERATOR that the host steps, `createMemo(() => createRoot(function* () { <site>; yield 1; }))` and
//                     `createEffect(() => m(), (it) => { it.next(); })`: its body runs after the root's owner is gone (F3)
//             genlayer   the site is in an ARROW the generator's body calls, `function* g() { const f = () => { <site> }; f(); yield 1; }`, with the generator and its iterator
//                     (`const it = g()`) written at MODULE scope, which the host steps with a `for...of`: the generator's own body holds no site, and the test of the
//                     innermost function saw a plain arrow (B1, the re-review of #220). At module scope because the harness's entry hands every export of App.tsx to the
//                     real `render`, a hand-off B3 (ADR 0967) answers with a row for everything written inside an exported component, on every build: the run of a
//                     generator whose call is at module scope has no function around it, so no row but the one the fix files stands there
//             genuntrack the site is in an `untrack` callback the generator's body runs in place, `function* g() { untrack(() => { <site> }); yield 1; }` (module scope too), which
//                     the host steps by spread (`[...it]`): the same layer, another in-place form (B1)
//             genhelper  the site is in a function written OUTSIDE the generator and called from its body, `function h() { <site> } function* g() { h(); yield 1; }` (module scope,
//                     the iterator made inside a module-level `createRoot` so the helper has an owned context), which the host steps with a `for...of`: the generator is no
//                     lexical layer of the site, and the caller walk of the entry test followed the call out to a generator whose body it took for a placed run (N1, the
//                     check of B1)
//             rootself   the site is in the callback of `createRoot` that is a NAMED function expression handing itself on, `let s; createRoot(function f() { if (!s) { s = f; return; } <site> })`
//                     (module scope: the root runs once there and saves `f`), which the host calls, `s!()`: a run the primitive does not make has no owner, and the entry test
//                     stopped at the owner-creating layer before it read the function's references (N2, the check of B1)
//             prop    the site is in the body of an ARROW written in a component element's prop, `<Kid run={() => { <site> }} />`, and `Kid`'s apply calls it (`props.run()`): the arrow's
//                     body runs on the apply's stack, and the JSX creation inside it was no site because it was written inside a component element (F2, the review of #225)
//             slot    the site is the GETTER of a component element's prop, `<Kid slot={<site>} />`, and `Kid`'s apply reads it (`void props.slot`): the getter runs where it is read, on
//                     the apply's stack (the getter proper of F2: a JSX creation written as a prop's value was no site either)
//             children  the same getter written as the element's children, `<Kid>{<site>}</Kid>`, which the apply reads as `props.children`
//             slotcall, childcall  the site is in an arrow the getter's own evaluation CALLS, `<Kid slot={(() => { <site>; return 1; })()} />` and `<Kid>{(() => { <site>; return 1; })()}</Kid>`: the
//                     call runs where the component reads the prop, though a function is written between the element and the site (the first fix of F2 took any function for one the
//                     element hands over)
//   hosts   apply (an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK), every route; tracked and settled (SC3001's leaf owners: PRIMITIVE_IN_FORBIDDEN_SCOPE,
//           CLEANUP_IN_FORBIDDEN_SCOPE), the `view` route only (the leaf codes rest on the owner graph's silence the component-render premise reached)
//
// The twin registers nothing and calls nothing (a handler that runs an `onCleanup` is NO_OWNER_CLEANUP, a code of its own), so it is the misuse text with no site run. Recall
// contexts, the direct binding bare (`erShape`), not crossed by the census, out of api-fire's plan.
import { cbProp, contain, decl, domExpr, expr, fn, iife, jsx, namedButton, num, prim, propEl, ref, ret } from "./model.mjs";

export const ER_ROUTES = ["view", "array", "saved", "timer", "generator", "root", "genlayer", "genuntrack", "genhelper", "rootself", "prop", "slot", "children", "slotcall", "childcall"];
export const ER_HOSTS = ["apply", "tracked", "settled"];
/** The (route, host) pairs the grid places: every route on an apply, and the `view` route on the two leaf hosts. */
const PLACED = [...ER_ROUTES.map((route) => [route, "apply"]), ["view", "tracked"], ["view", "settled"]];
/** The hosts and routes each code is judged in: the codes the host's callback reports for a site in it. */
export const ER_CODES = {
  PRIMITIVE_IN_EFFECT_CALLBACK: { hosts: ["apply"], routes: ER_ROUTES },
  PRIMITIVE_IN_FORBIDDEN_SCOPE: { hosts: ["tracked", "settled"], routes: ["view"] },
  CLEANUP_IN_FORBIDDEN_SCOPE: { hosts: ["tracked", "settled"], routes: ["view"] },
};
/** `{ [context]: "misuse" | "legal" }`. */
export const ER_CONTEXTS = Object.fromEntries(PLACED.flatMap(([route, host]) => [[`er-${route}-${host}`, "misuse"], [`er-${route}-${host}-never`, "legal"]]));
export const ER_CONTEXT_NAMES = Object.keys(ER_CONTEXTS);

const NAME = new RegExp(`^er-(${ER_ROUTES.join("|")})-(${ER_HOSTS.join("|")})(-never)?$`);
/** `{ route, host, never }` of an `er-*` context name, or null. */
export const erParts = (context) => {
  const match = NAME.exec(context);
  return match && PLACED.some(([route, host]) => route === match[1] && host === match[2]) ? { route: match[1], host: match[2], never: Boolean(match[3]) } : null;
};

/** The base shapes an `er-*` cell takes: the direct binding, no layer. */
export function erShape(context, shape) {
  return !erParts(context) || (shape.binding === "direct" && (shape.layer ?? "none") === "none");
}

/** The host's own placement of `held` (the plain primitive-layer hosts): the statements its callback runs. */
function placeIn(host, held) {
  switch (host) {
    case "tracked": return expr(prim("tracked", fn(held)));
    case "settled": return expr(prim("settled", fn(held)));
    case "apply": return expr(prim("effect", fn([ret(num(1))]), { apply: fn(held) }));
    default: throw new Error(`no entry-by-reference host ${host}`);
  }
}

/** The expression the site's statement holds (`expr(e)`): the `slot` and `children` routes hand it to a component element as a prop's value or as its children. */
function siteExpr(stmts) {
  if (stmts.length !== 1 || stmts[0].k !== "expr") throw new Error("an entry-by-reference getter route takes one expression statement as its site");
  return stmts[0].e;
}

/** The placement of `stmts` for an `er-*` context (`{ body, view }`, and `hoisted` for module code), or undefined for any other. */
export function erContext(context, stmts, kit) {
  const parts = erParts(context);
  if (!parts) return undefined;
  const { route, host, never } = parts;
  const h = kit.name("h");
  const pk = kit.name("pk");
  // What the host's callback does with a holder: calls it (`text`), or, in the twin, only keeps the holder's name used.
  const runs = (holder, text) => [expr(domExpr(never ? `void ${holder}` : text))];
  switch (route) {
    case "view": {
      // A function that returns JSX and that no tag renders, held by a variable: its entry is a call through the variable.
      const held = decl(h, fn([...stmts, ret(jsx())], { form: "function" }));
      return { body: [held, expr(domExpr(`const ${pk}: (() => unknown) | undefined = ${h}`)), placeIn(host, runs(pk, `${pk}!()`))], view: [] };
    }
    case "array": {
      // The handler is registered, and an array also holds it.
      const all = kit.name("all");
      const body = [decl(h, fn(stmts)), decl(all, domExpr(`[${h}]`)), placeIn(host, runs(all, `${all}[0]!()`))];
      return { body, view: never ? [] : [namedButton(kit.name("b"), ref(h))] };
    }
    case "saved": {
      // The handler is registered, and assigned to a variable the host calls.
      const body = [decl(h, fn(stmts)), expr(domExpr(`let ${pk}: (() => void) | undefined`)), expr(domExpr(`${pk} = ${h}`)), placeIn(host, runs(pk, `${pk}!()`))];
      return { body, view: never ? [] : [namedButton(kit.name("b"), ref(h))] };
    }
    case "timer": {
      // The timer runs it later; the host calls it through the variable now.
      const body = [decl(h, fn(stmts)), expr(domExpr(`let ${pk}: (() => void) | undefined`)), expr(domExpr(`${pk} = ${h}`)), ...(never ? [] : [expr(domExpr(`setTimeout(${h}, 1000)`))]), placeIn(host, runs(pk, `${pk}!()`))];
      return { body, view: [] };
    }
    case "generator": {
      const it = kit.name("it");
      const frame = contain("frame", 9, stmts, { head: `const ${it} = untrack(function* () `, close: ");", after: ["yield 1;"], imports: ["untrack"] });
      return { body: [frame, placeIn(host, runs(it, `${it}.next()`))], view: [] };
    }
    case "root": {
      // The memo's value is the generator the root's callback is: the apply, which gets it as its parameter, steps it, so its body runs on the apply's stack and under no owner.
      const m = kit.name("m");
      const frame = contain("frame", 9, stmts, { head: `const ${m} = createMemo(() => createRoot(function* () `, close: "));", after: ["yield 1;"], imports: ["createMemo", "createRoot"] });
      const effect = never ? `createEffect(() => ${m}(), () => { void ${m}; })` : `createEffect(() => ${m}(), (it) => { it.next(); })`;
      return { body: [frame, expr(domExpr(effect, undefined, ["createEffect"]))], view: [] };
    }
    case "genlayer": {
      // The generator's body calls an arrow that holds the site; the host steps the generator with a `for...of`, which no SC9012 covers (`it.next()` is a call the checker cannot
      // follow). The generator and its iterator are module code (`hoisted`).
      const g = kit.name("g");
      const f = kit.name("f");
      const it = kit.name("it");
      const frame = contain("frame", 9, stmts, { head: `function* ${g}() { const ${f} = () => `, close: `; ${f}(); yield 1; }` });
      return { hoisted: [frame, expr(domExpr(`const ${it} = ${g}()`))], body: [placeIn(host, runs(it, `for (const _ of ${it}) { void _; }`))], view: [] };
    }
    case "genuntrack": {
      const g = kit.name("g");
      const it = kit.name("it");
      const frame = contain("frame", 9, stmts, { head: `function* ${g}() { untrack(() => `, close: `); yield 1; }`, imports: ["untrack"] });
      return { hoisted: [frame, expr(domExpr(`const ${it} = ${g}()`))], body: [placeIn(host, runs(it, `void [...${it}]`))], view: [] };
    }
    case "genhelper": {
      // The helper is written beside the generator, inside a module-level `createRoot` (so the graph holds an owned context for it and no function stands around the
      // root's callback for B3 to ask about), and called from the generator's body; the root makes the iterator (the body does not run yet) and the host steps it.
      const hh = kit.name("hh");
      const g = kit.name("g");
      const it = kit.name("it");
      const frame = contain("frame", 9, stmts, { head: `createRoot(() => {\n  function ${hh}() `, close: `\n  function* ${g}() { ${hh}(); yield 1; }\n  ${it} = ${g}();\n});`, imports: ["createRoot"] });
      return { hoisted: [expr(domExpr(`let ${it}: Generator<number> | undefined`)), frame], body: [placeIn(host, runs(it, `for (const _ of ${it}!) { void _; }`))], view: [] };
    }
    case "rootself": {
      // The root runs once at module scope and saves its callback (a named function expression); the host calls the saved function, which runs on its own stack with no owner.
      const f = kit.name("f");
      const sv = kit.name("sv");
      const guard = expr(domExpr(`if (!${sv}) { ${sv} = ${f}; return; }`));
      const frame = contain("frame", 9, [guard, ...stmts], { head: `createRoot(function ${f}() `, close: ");", imports: ["createRoot"] });
      return { hoisted: [expr(domExpr(`let ${sv}: (() => void) | undefined`)), frame], body: [placeIn(host, runs(sv, `${sv}!()`))], view: [] };
    }
    case "prop": {
      // The arrow is written in the element's prop and `Kid`'s apply calls it (model.mjs `cbProp`, call `apply`); the twin's `Kid` never calls it.
      return { body: [], view: [cbProp(kit.name("v"), fn(stmts), never ? "never" : "apply", "inline")] };
    }
    case "slot": case "children": case "slotcall": case "childcall": {
      // `Kid` is written at module scope, reads the getter in its apply (the twin reads nothing, so the getter never runs), and the component renders it with the site as the
      // prop's value or as its children.
      const kid = kit.name("KidG");
      const inSlot = route === "slot" || route === "slotcall";
      const read = never ? "void props" : inSlot ? "void props.slot" : "void props.children";
      const text = `(props: { slot?: unknown; children?: unknown }) => { createEffect(() => 1, () => { ${read}; }); return <p>k</p>; }`;
      // `slotcall` and `childcall` hold the statements in an arrow called in place; the others hand the site's own expression over.
      const value = route === "slotcall" || route === "childcall" ? iife(fn([...stmts, ret(num(1))])) : siteExpr(stmts);
      return { hoisted: [decl(kid, domExpr(text, undefined, ["createEffect"]))], body: [], view: [propEl(kid, value, "slot", !inSlot)] };
    }
    default: throw new Error(`no entry-by-reference route ${route}`);
  }
}

/** `{ [code]: { misuse, legal } }` for SPEC. */
export function erPlacement() {
  return Object.fromEntries(Object.entries(ER_CODES).map(([code, { hosts, routes }]) => {
    const contexts = ER_CONTEXT_NAMES.filter((name) => hosts.includes(erParts(name).host) && routes.includes(erParts(name).route));
    return [code, { misuse: contexts.filter((name) => ER_CONTEXTS[name] === "misuse"), legal: contexts.filter((name) => ER_CONTEXTS[name] === "legal") }];
  }));
}
