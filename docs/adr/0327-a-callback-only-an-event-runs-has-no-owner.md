# ADR 0327: A callback only an event runs has no owner

- Status: accepted and implemented (2026-10-19).
- Owners: `unowned_event_literals` and the owner mode of `InvocationFlow`
  (`owner_site_role`, the refused primitive slot in `uses`)
  (`solid-reactive-ir/src/execution_role.rs`); `SemanticLookup::unowned_event_literals`
  and the `unowned-event-literals` entry of `cross_file_proof_digest`
  (`indexes.rs`); the seed in `owner_node` (`owners.rs`).
- Relation: closes the SC4001 gap ADR 0319 recorded ("the owner rules do not
  consume this role"). Uses ADR 0319's walk in a stricter mode; the read role
  is unchanged.

## Context

The runtime oracle (2026-10-19, `oracle-owner-flow`,
`oracle-owner-flow-import`, and `c6-leak`, `r4c-leak` from the ADR 0319
review rounds) found unproven clean claims:

```tsx
function handlers(cb: () => void) {
  return { onClick: () => cb() };
}
const h = handlers(() => { createEffect(…); onCleanup(…); });
return <button {...h}>b</button>;
```

Solid warns `NO_OWNER_EFFECT` and `NO_OWNER_CLEANUP` on click. Written on the
button directly, the same code is a proven `SC4001`, because the compiler seeds
an event handler unowned. The owner graph gives a literal context only through
named calls, compiler event roles and primitive callback edges, so a literal
invoked through a parameter (`cb()`) had no context and reported nothing.

ADR 0319's read answer cannot be reused. `createRoot(fn)` and
`runWithOwner(o, fn)` run `fn` in their caller's *read* window, so the read
walk answers `EventCallback` for a callback a handler runs inside either of
them. They supply an owner, so seeding from that answer reported both as
proven violations (probed: `oracle-owner-flow-owned`).

## Decision

1. **Owner walk.** The same forward walk, with two changes:
   - any argument slot of a primitive refuses;
   - a call's role is never read from the semantic role at the call. It is
     `EventCallback` when the function the call is written in carries a
     compiler event-handler fact. Otherwise it is the walk's own answer for
     that function, in the same call context.

   Intrinsic `on…` attributes and spread keys answer `EventCallback`, as in
   ADR 0319. Every invocation must answer `EventCallback`.
2. **Seed.** The owner node of such a literal is seeded unowned and proven
   unowned, as a compiler event handler is. An owner primitive directly in it
   is then a proven `SC4001`. Owner primitives in its nested callbacks follow
   the existing edges.
3. **Cache.** The set of such literals is computed once per build and bound in
   the cross-file proof digest next to ADR 0319's answers.

## Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| closed program, no `eval` | as ADR 0319 | ADR 0193 | `helper_event_edits_match_cold` (open boundary: no finding) |
| the literal is a plain function | not a component or generator | as ADR 0319 | — |
| every invocation is an event dispatch | intrinsic `on…` attribute or spread key, or a compiler event-handler function, reached by direct calls only | the host dispatches with no owner (rc.13 runtime: `NO_OWNER_*`) | `Spread`, `Attribute`, `Imported` |
| no owner is supplied on the way | every primitive slot refuses; no read-role shortcut | `createRoot` and `runWithOwner` supply one | `UnderOwner`, `UnderRoot` |
| mixed invocations refuse | one non-event invocation refuses the literal | a component-body call has an owner | `AlsoInBody`; the `LIB_BODY` edit |
| an await ends the walk | a call after an await in an async function refuses | the continuation is not the dispatch | inherited from ADR 0319 |
| a cached answer is current | the set is in the cross-file digest | the proof reads the helper's file | `helper_event_edits_match_cold` |

## Consequences

Corpus A/B (with ADR 0326): +1 `SC4001` violation and +5 `SC4001`
uncertifiable.
- The violation is in app-game `solid-dockview`. `createTrackedEffect` runs in
  `createDocumentInGroup`, which an `onAddPanel` arrow calls, and
  `MyRightHeaderActions` gives that arrow as a `div`'s `onClick`. It is a real
  missing owner.
- The uncertifiables are `createEventListener` calls in a handler object
  spread onto a button (app-game `docking`). The handler is unowned; whether
  the package needs an owner is not proven.

The oracle covers all five owner sites the first run listed as open.

Not covered: a literal that is sometimes run by an event and sometimes
elsewhere refuses as a whole, even though one unowned path would prove the
defect. A call after an await refuses too. Both stay silent, as before.
