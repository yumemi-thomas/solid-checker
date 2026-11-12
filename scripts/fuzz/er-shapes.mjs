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
//   hosts   apply (an effect's apply: PRIMITIVE_IN_EFFECT_CALLBACK), every route; tracked and settled (SC3001's leaf owners: PRIMITIVE_IN_FORBIDDEN_SCOPE,
//           CLEANUP_IN_FORBIDDEN_SCOPE), the `view` route only (the leaf codes rest on the owner graph's silence the component-render premise reached)
//
// The twin registers nothing and calls nothing (a handler that runs an `onCleanup` is NO_OWNER_CLEANUP, a code of its own), so it is the misuse text with no site run. Recall
// contexts, the direct binding bare (`erShape`), not crossed by the census, out of api-fire's plan.
import { contain, decl, domExpr, expr, fn, jsx, namedButton, num, prim, ref, ret } from "./model.mjs";

export const ER_ROUTES = ["view", "array", "saved", "timer", "generator", "root", "genlayer", "genuntrack"];
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
