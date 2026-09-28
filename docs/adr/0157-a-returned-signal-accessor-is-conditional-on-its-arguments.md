# ADR 0157: A returned signal accessor is inert only on its arguments' terms

- Status: **proposed** (2026-09-29). Not implemented: it needs an owner decision
  (below) before any vocabulary is built.
- Date: 2026-09-29
- Owners, when implemented: the semantic model (`ValueShape`,
  `DescribedCall`), the `returns` census and its dependency obligations
  (`type_facts.rs`, `dependencies.rs`), the dialect's inert-read row
  (`Dialect::inert_accessor_read`), the generator's returns walk, and the
  consumer's projection (`contracts.rs`)
- Relation: the lead asked, on 2026-09-28, for a `returns` form for a returned
  `[accessor, setter]` tuple from an audited or composed `createSignal`, and
  for element-of-result forwarding (`dep()[0]`, `const [a] = dep(); return a`),
  to close `createMediaQuery` → `createHydratableSignal` → `createSignal`
  stating returns-accessor. This ADR records why neither form can be certified
  as specified, and what would have to be decided instead. It extends ADR 0146,
  whose row is the premise both forms rest on.

## Context

### The premise both forms rest on

ADR 0146 describes a returned accessor as `reads: [owned-signal]`,
`returns: [read-value]` only for an **inert** read, and the dialect row that
states inertness (`solid_2.rs` `inert_accessor_read`, `createSignal`'s tuple
item 0) holds only when **every argument of the creating call is a primitive
by grammar**: no function first argument, no options object. The reason is the
published bytes: `@solidjs/signals@2.0.0-rc.9` `createSignal(e, t)` takes the
writable-memo path when `typeof e === "function"`, and an options object can
carry `equals` and `unobserved`, which run caller code.

### `createHydratableSignal` does not meet it, by its own declaration

`@solid-primitives/utils@7.0.0-next.4`:

```js
function createHydratableSignal(serverValue, update, options) {
  if (isServer) return createSignal(serverValue, options);
  if (sharedConfig.hydrating) {
    const [state, setState] = createSignal(serverValue, options);
    onSettled(() => { setState(() => update()); });
    return [state, setState];
  }
  return createSignal(update(), options);
}
```

declared `createHydratableSignal<T>(serverValue: T, update: () => T, options?:
SignalOptions<T>)`. `T` is unconstrained, so a caller `tsc` accepts may pass a
function. Probed against the published bytes (solid-js, `@solidjs/signals`,
`@solidjs/web` 2.0.0-rc.9, utils 7.0.0-next.4, in an isolated `$TMPDIR`
install), `createHydratableSignal(fn, () => fn)` hands back an accessor whose
value is `fn`'s **result**, `"computed"`, under the default, `node` and
`browser` conditions alike: the read is a memo over the caller's code, not the
value written. And with `{ equals }` in `options`, a write runs the caller's
`equals` under `browser`. So an unconditional claim "element 0 is an inert
accessor over the signal this call created" is false for inputs the
declaration admits. No composition can restate a claim the dependency cannot
certify, and the dependency cannot certify this one.

The setter has the same gap from the other side: called with a function it
runs it (the updater form), so it is not a described callable in ADR 0145's
sense (`callbacks: []`), and `DescribedCall` has no `writes` domain. ADR 0145
names "a nested `callbacks` item" as vocabulary not yet built.

### Demand, measured on the checkpoint corpus

Every returned tuple in the 97 packages' published `dist/` (`rg` over the
unpacked tarballs):

| shape | instances | inert by ADR 0146's row |
| --- | ---: | --- |
| `return createSignal(…)` or destructure-and-rebuild of one | 4: `trigger`'s `createSignal(void 0, triggerOptions)`, and utils' own three in `createHydratableSignal` | **none**: an options object, or the caller's arguments |
| any other `return [a, b]` | 26 (controlled-signal, date, cookies, static-store, websocket, vibrate, ...) | not a signal tuple of an audited call: a `createSignal(fn, …)` memo, a custom setter, a store, a cleanup or a `noop` pair |
| `const [x] = createHydratableSignal(args); return x` | 5: media `createMediaQuery`, focus `createFocusSignal`, connectivity `createConnectivitySignal`, active-element, page-utilities | only where the **call site's** arguments meet it |

So the tuple form as specified has no certifiable instance, and the five real
dependents all need the *dependency's* claim to be conditional on arguments
that only the *dependent's* call site fixes:

| dependent | arguments at the call site | inert on the row's terms |
| --- | --- | --- |
| focus `createFocusSignal` | `false`, `() => document.activeElement === target` | yes: a literal, and a literal arrow whose completion is a primitive by grammar |
| media `createMediaQuery` | `serverFallback` (a parameter, declared `boolean`), `() => mql.matches` | only under the declared-signature premise for `serverFallback`, and `mql.matches` is not a primitive by grammar |
| connectivity | `true`, `() => navigator.onLine`, `{ ownedWrite: true }` | no: an options object |
| active-element | `null`, `getActiveElement` | no: `update` is a helper, not a literal |
| page-utilities | `true`, `checkVisibility`, `INTERNAL_OPTIONS` | no |

`createMediaQuery`'s other branch, `if (isServer) return () => serverFallback`,
is ADR 0145's described callable over a captured parameter, which refuses today
(not a primitive by grammar), and ADR 0113's amendment declined an unwritten
parameter's type as return evidence.

## Proposal (for the owner)

**A returned signal accessor may be described with the premise it is inert
under, stated in terms of the export's own arguments, and a dependent restates
it only where its call site discharges the premise.**

1. **The dependency's claim is parametric.** `createHydratableSignal` would
   close `returns` over a tuple whose element 0 is ADR 0146's inert accessor
   *over a value that is argument 0 or the result of invoking argument 1*,
   with the premise "that value is not callable, and argument 2 is absent".
   The census proves the body under the premise (every `createSignal` call's
   first argument is that parameter or that invocation's result; no other
   argument), exactly as ADR 0116's `invocation-result` and ADR 0115's argument
   containers name a caller's value by position. Element 1 needs its own
   description, or the tuple states element 1 as undescribed and the claim is
   about element 0 only (a partial tuple, which the model does not have today).
2. **The dependent discharges the premise at its call site** from its own
   facts: argument 0 a primitive by grammar (focus's `false`), argument 1 a
   literal whose every completion is a primitive by grammar, no argument 2.
   Element-of-result forwarding (`const [x] = dep(…); return x`) then restates
   element 0, and a conditional return closes only when every live branch
   closes with its own evidence; a branch that is open keeps the whole return
   open. `createFocusSignal` would certify returns-accessor; `createMediaQuery`
   would not, until the owner also decides the next question.
3. **Whether a declared parameter type discharges a premise.** ADR 0038 already
   classifies the `creates` census under the export's declared signature (a
   caller `tsc` rejects is out of scope), while ADR 0113's amendment declined a
   parameter's type as return evidence. `createMediaQuery` needs the former
   reading for `serverFallback` (declared `boolean`) at both of its branches.

### What the owner is asked

- May a returns claim carry an argument premise (1), and may a dependent
  discharge it at the call site (2)? This is new vocabulary: a premise on a
  closed claim, carried in the contract and checked by the consumer or the
  dependent's census.
- Does element 1 have to be described for the tuple to close, or is a claim
  about element 0 alone acceptable?
- May a declared `.d.ts` parameter type discharge an inertness premise (3), as
  ADR 0038's declared-signature premise does for `creates`?

## Consequences of recording this now

Nothing is implemented and nothing moved: no code, fixture, protocol or tier
change. The next step is the owner's answer; with (1) and (2) alone the
measured ceiling is one dependent (`createFocusSignal`), with (3) two.
