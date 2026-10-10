# ADR 0178: An open `returns` claim keeps its described return

- Status: accepted and implemented (2026-10-04). Second lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners: the generator's obligation marking
  (`mark_summary_claims_unknown`, `unify_runtime_alias_summaries`) and the
  proposal emitter (`described_return_operations` in `inferred_contract.rs`).
  The certification census is unchanged.
- Relation: the `returns` counterpart of ADR 0174, which keeps an open
  owner-requirement list's guaranteed items. No new wire field:
  `ContractExport::open_return` is generation only.

## Context

An unresolved call (a dependency export with no accepted contract, or any
obligation the attribution falls back on) marks every claim of the export
unknown. For `returns` that discarded what the export's own return statement
hands back. The 2026-10-04 graph-lane measurement found import widenings
behind 270 of 271 all-open exports.

`createKeyHold` is the plain case:

```js
function createKeyHold(key, options = {}) {
  if (isServer) return () => false;
  const heldKey = useCurrentlyHeldKey();
  if (preventDefault) makeEventListener(window, "keydown", ...);
  return createMemo(() => heldKey() === key);
}
```

`makeEventListener` comes from another package and opened everything, so the
generator proposed no return. A user reading the memo at a component's top
level got `STRICT_READ_UNTRACKED` at runtime and nothing from the checker.

## Decision

1. **Opening `returns` keeps the described return.** When an obligation
   opens a `Known(Some(return))` claim, the return moves to `open_return`.
   An unknown call can add behavior to the export. It does not replace the
   value a `return` statement hands back. `Known(None)` keeps nothing.
2. **Aliases keep a return only when they agree.** An open union of runtime
   aliases keeps a return only if every alias states the same one, described
   or retained.
3. **The emitter proposes it as an item, never a closure.** An open claim
   with a retained return publishes the same operations a described return
   would (ADR 0146's reading callables, or its shape), under a partial
   `returns` knowledge set. The valueless-completion walk still answers
   first.
4. **The census decides.** Nothing about certification changes. A returned
   accessor still needs its witness, for example an owned accessor this call
   created and handed back unaltered, traced by the producer. A value that
   actually came from the unresolved call has no such witness and is
   withdrawn.

## Consequences

- The open domain still says "something else may be returned": it never
  closes. Consumers read the positive item, which is all a strict-read
  finding needs.
- `useKeyDownList` and `useCurrentlyHeldKey` return through `rootless`'s
  `createSingletonRoot`, a call result from another package. They remain
  unproposed.

## Evidence

- Unit tests:
  - `opening_returns_keeps_the_described_return`: marking and the alias
    union;
  - `an_open_returns_claim_publishes_its_retained_return_as_an_item`: the
    emitter.
- Generator: `@solid-primitives/keyboard@2.0.0-next.5` `createKeyHold` now
  proposes `{"kind": "reactive", "role": "accessor"}`; before, it proposed no
  return.
- Browser tier regenerated with `--carry`:
  - the misuse ledger goes from 63 to 66 static violations of 123:
    `createKeyHold`, `createConnectivitySignal` and `createMediaQuery`. The
    last two return `@solid-primitives/utils`' `createHydratableSignal`
    accessor, now proposed and proven through the dependency's contract;
  - each new violation is runtime-detected (`STRICT_READ_UNTRACKED`);
  - no correct twin is flagged;
  - no checkpoint row changes status.
