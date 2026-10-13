# ADR 0292: A `lazy` module's `default` is rendered, not retained

- Status: accepted (2026-10-13).
- Owners: `Dialect::renders_loaded_default_argument` (`solid-dialect`), its
  Solid 2 answer and the `lazy_renders_component` release digit
  (`solid_2.rs`, `solid_2/releases.rs`), the witness census
  `owners::lazy_default_render_witnesses`, and its one consumer
  `interproc::runs_in_unrendered_object_callable`.

## Context

app-game's `packages/math-examples/rapier-2d-physics-engine/index.tsx`
declares `TestCube` inside `lazy(async () => { … return { default: TestCube };
})` and calls `createRAF`'s `start()` in its body. Contract mode reports the
SC1001; under `--dependency-source` (ADR 0284) the transported tuple read
(ADR 0282) was uncertifiable with the consumer-timing message. ADR 0288 left
this site open.

`TestCube` is already a proven component (it returns JSX directly), and its
body already has the untracked rendering role. The refusal is
`runs_in_object_callable`: an object property whose value is a reference to an
enclosing function makes every summarized call in that function's body take
forced callback timing, because a JSX-returning registry member can be proven
a component without ever being rendered. `{ default: TestCube }` is such a
property. The contract-backed read path never asks the question, which is why
the two modes disagreed (research: `rust/target/research/raf-lazy-gap.md`).

## Runtime authority

`solid-js@2.0.0-rc.13` (`dist/solid.js:1349`, `dist/solid.dev.js:1401`, the
observe build alike; `dist/server.js` selects the same member):

- `lazy(fn, options, moduleUrl)` calls `fn` only from `load`, which the
  returned wrapper and `wrap.preload` reach;
- the member selected is `options?.export ? mod[exportName] : mod.default`;
- the wrapper renders it as `createComponent(Comp, props)` inside a memo, and
  `observedComponent` runs the body as `untrack(() => Comp(props), label)`
  (`dist/solid.dev.js:59`): the strict-read window a component rendered by its
  tag has;
- `wrap.preload = load` hands the module promise to any caller.

The server builds call the member as `cur.v(props)` inside the server memo,
as their `createComponent` calls `Comp(props)`; neither opens a strict-read
window, and the checker already gives a server-rendered body the role a
browser-rendered one has (a plain and a lazy body report alike under
`--runtime-target node --rendering string-ssr`).

rc.3 and rc.9 render the member as `untrack(() => Comp(props))` themselves,
with no `createComponent` and no strict-read label (rc.9
`dist/solid.dev.js:1175-1179`), so their loaded body opens no strict-read
window. The two `lazy` overloads are byte-identical in rc.9 and rc.13
`types/client/component.d.ts`.

## Decision

1. **A dialect question.** `Dialect::renders_loaded_default_argument(primitive,
   arguments)` names the loader slot of a call whose returned component, when
   rendered, renders the loader's module `default` as a component. The default
   is `None`. Solid 2 answers `Some(0)` for a one-argument `lazy` only on a
   vocabulary whose `solid-js` is rc.13 (a new release digit,
   `lazy_renders_component`, token `lazy-renders-component`; the audited
   triple's dialect id gains it). A second argument selects a member by data,
   so it answers `None` rather than letting the engine guess.
2. **A census of witnesses.** `owners::lazy_default_render_witnesses`
   computes, once per project, the `(function, property)` pairs where all of
   these hold:
   - the call resolves exactly to that primitive, has no spread, and its slot
     holds (under transparent wrappers) an `async`, non-generator function
     literal;
   - the loader's one unconditional own return (`accessor_provenance::
     single_return`) is a closed object literal (`closed_properties`: data
     properties only, no spread, computed or duplicate key, no `__proto__`),
     whose `default` value is a plain identifier under transparent wrappers;
   - the identifier names a function declared directly in the loader's body,
     either a function declaration or a `const` initialized with the literal
     itself, unwritten (`binding_written`, `function_value_is_current`),
     referenced nowhere else, synchronous, non-generator, and
     `ComponentStatus::Proven`. Capitalization is not evidence;
   - the call initializes an unexported, unwritten `const` whose every
     reference is a JSX tag name (opening or closing, never dotted), with at
     least one opening tag. Any other use, `.preload()` above all, may reach
     the module object;
   - no analyzed file holds a reflective write through a `prototype` (the
     visible veto ADR 0288 kept for a fresh literal).
3. **One disposition.** At `interprocedural_result_reads_for_file`, the object
   callable test becomes `runs_in_unrendered_object_callable`: the same
   witnesses, less a census witness, and only for a call written directly in
   that function's own body (`written_directly_in`). Every other witness, an
   enclosing function's included, stays in force. The raw predicate keeps its
   other consumer (`accessor_provenance`'s array members). Component
   discovery, tuple transport, the loader's own execution role and the
   contract path are unchanged.

### Why it is sound

The module object is a fresh literal returned by a loader only the `lazy`
call holds. The wrapper reaches it only through `load`, and nothing else can,
because every wrapper reference is a tag. The function's one reference is that
property, so the runtime's `createComponent(mod.default, props)` is its only
invocation, which is exactly the role its body already has. Premises, all
standing: unpatched `Promise`/`Object` built-ins (ADRs 0167, 0190, 0211, 0262)
with ADR 0288's visible prototype veto, and standard runtime configuration
(ADR 0266), including a hydration host whose preloaded modules are the
loader's.

## Consequences

Fixtures `reactive-ir/lazy-default-consumer` (4 SC1001 sites move from
uncertifiable to violation; 13 refusals stay uncertifiable, 2 stay silent; the
direct read and 2 unrelated pre-existing violations are pinned) and
`reactive-ir/lazy-default-consumer-rc9` (nothing moves on rc.9). No other
coverage snapshot moved.

Not claimed:
- `lazy(fn, { export })` and a loader that is a reference, not a literal;
- a module object built elsewhere, aliased, spread, or selected by a
  condition; a non-`async` loader returning a promise;
- a wrapper that is exported, preloaded, aliased or passed anywhere, and a
  cross-file wrapper (it would have to be exported);
- a component declared outside its loader, or returning no JSX (identity
  discovery is unchanged);
- a read in a callback nested in the loaded body keeps its refusal;
- `{ default: flag ? A : B }` never met the retention guard and was already a
  violation; this ADR neither covers nor changes it.
