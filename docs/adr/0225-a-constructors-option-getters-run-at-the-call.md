# ADR 0225: A constructor's option getters run at the call

- Status: accepted and implemented (2026-10-07).
- Owners:
  - the protocol words: `schema/solid-reactivity.schema.json`,
    `contract_semantics` (model, validation, canonical tags, consumer);
  - their instantiation at a call: `solid-reactive-ir/src/property_gets.rs`,
    with the descriptor-aware literal facts `AstFacts::object_get_shapes`
    (AST facts schema 51);
  - authoring: `scripts/lib/property-get-contracts.mjs` and the spec
    `@tanstack+virtual-core@3.17.8`.
- Relation: an additive extension of the stable `schemaVersion: 1` package
  contract. Existing documents, digests and receipts keep their meaning and
  are not re-emitted.

## Context

Five runtime-confirmed twins stayed uncertifiable because the contract
format could not say that a package call reads its caller's option getters.
Examples: the `Virtualizer` constructor copies its options with `for…in`, and
`table-core` spreads them. The existing `get` protocol means an unspecified
property, enumeration or descriptor operation on a bare argument, so it can
never select a getter body. Two research passes reached this independently
(`rust/target/research/contracts/tanstack-*`), and the owner authorized a
richer schema.

## Decision

1. **Two protocol words state a value copy of argument N.** Each counts
   enumeration entries, not property Gets:
   - `get-enumerable-string-values`: a `for…in` copy (enumerable string keys,
     inherited ones included), entered once per call;
   - `get-own-enumerable-values`: an object-spread copy (own enumerable
     strings and symbols), entered at most once, because nothing yet proves
     that what precedes the spread completes.

   Both require a bare parameter source, call trigger, same-stack schedule,
   ambient tracking and owner, and no guard.
2. **A getter runs at the call that copies it, with that call's role.** Its
   reads are attributed to the package call, with the getter's own read as
   the origin. No permanent role is assigned to the getter declaration, so a
   construction inside `untrack` stays clean while the same object spread in
   a tracked compute is unaffected.
3. **Only an exact receiver binds.** That is a fresh closed object literal,
   or one immutable local `const` initialized by one, with no alias, export,
   descriptor change or earlier spread. A later same-frame spread of it, or
   one inside a function literal handed to a later call, is allowed.

   Literals with an internal spread, computed or numeric keys, duplicates,
   setters or `__proto__` are open. A refused receiver is an explicit
   `unbound-contract-claims` obligation, never a clean result.
4. **Only the first getter is guaranteed.** It is the first getter of the
   literal, read through an accessor call created by a dialect source
   primitive, at its entry call. A later getter runs only if earlier ones
   complete, so it is possible, not guaranteed. A member-only getter body
   remains an obligation.
5. **The probe gate binds a new-protocol result** to its full claim, pair
   bytes, runtime integrities and case identity. A broad-`get` probe result
   can never satisfy it, and the certified census refuses the new words
   rather than guess a witness.

## Consequences

- Twins: of 38, 25 are proven and 13 uncertifiable (were 22 and 16).
  `ai-virtual-count-getter`, `probus-virtual-callback-option-getter` and
  `probus-virtual-scroll-margin-getter` are proven. No correct twin changed,
  and the base ledger is unchanged.
- rc.13 corpus: no violation moved. The real apps call `Virtualizer` through
  their own helper modules, which this slice does not follow.
- Not yet:
  - option objects passed through a helper;
  - `virtual-core` 3.17.10 and 3.17.11 (their identities are not
    generated);
  - `probus-table-data-getter-spread`: caller feature hooks run before the
    spread;
  - G6 (a stored callback on the current instance) and G7 (guarded query
    lifecycles), judged not worth their cost for one and two twins.

## Evidence

- Design and risk review by a Codex agent:
  `rust/target/research/schema-extension/` (`design.md`, `risks.md`).
- Integration found that the receiver resolver refused every named options
  object inside an exported function. An exported function's span contains
  its body, so the export check now applies to module-level bindings only,
  and a unit test pins both cases.
- Chrome probes on rc.13, against the `ai-memory-ui` install: the
  `Virtualizer` pair and its `count`, `scrollMargin` and `getScrollElement`
  pairs all raise `STRICT_READ_UNTRACKED` for the misuse twin and nothing
  for the correct one.
