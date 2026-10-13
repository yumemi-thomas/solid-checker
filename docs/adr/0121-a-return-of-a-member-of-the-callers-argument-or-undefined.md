# ADR 0121: A return of a member of the caller's argument, or undefined

- Status: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the Type Facts producer's return arms (`invocation_transcripts.go`,
  handshake protocol 64), the facts schema (43 -> 44, `optional_members`), the
  semantic model (`ValueShape::Undefined`), the generator's returns walk
  (`returns_walk.rs`, `demand_plan.rs`, `inferred_contract.rs`), the policy-2
  `returns` census, positive facts and synthesized veto (`type_facts.rs`,
  `synthesized_vetoes.rs`), and the consumer's `returns` projection
  (`contracts.rs`)
- Relation: widens ADR 0115/0116's enumeration of exact outputs by two values.
  Everything they decided about one `return` operation per value, the census
  premises and the consumer's reading of a union holds unchanged. Round 2 of
  the `callHandler` campaign (ways-to-improve § 3.3, § 4 step 6); ADR 0120 was
  round 1.

## Context

After ADR 0120, `@kobalte/utils`' `callHandler` (112 consumer sites) had
`callbacks` and `creates` closed and one open domain: its return,
`event?.defaultPrevented`. The completion is `any`; ADR 0113's plain return was
proposed and refused, correctly. The value is either whatever the caller's
event holds at `defaultPrevented` or, for a nullish event, `undefined`, and the
model could say neither.

## Decision

**A return may hand back a literal member of the caller's unchanged argument
(`parameter i` at `[key]`), or `undefined`.** Each is one more exact output
enumerated the ADR 0115 way: one `return` operation per value, closed by the
`returns` census from the producer's arms. `return p?.key` is two operations,
never a `choice`: a `choice` is a value closure of its own that no census
decides.

### The shapes

- The member reuses `ValueShape::Parameter { index, path }` with a one-segment
  path; its encoding is unchanged.
- `ValueShape::Undefined` is new, canonical tag 20, wire `"undefined"` or
  `{"kind": "undefined"}` with no other field. `Plain` says only "no reactive
  capability", so nothing existing meant exactly `undefined`. Documents that
  do not use it hash as before (the legacy golden vector is unchanged; a new
  one pins a document that does).

### The producer states the arms (protocol 64)

A return arm that is, after identity-preserving wrappers, a non-call property
access or literal-keyed element access of a parameter's unchanged, unwritten
binding states that parameter with the path (ADR 0120's literal-key rooting).
An optional chain `p?.key` decomposes into the member arm and an explicit
`undefined` arm. A path longer than one segment is stated and refused by the
census by name.

### The census

`arm_container` admits a one-segment member arm and the `undefined` arm; ADR
0115's premises hold (every live value is an enumerated container, every
enumerated container is handed back). It refuses by name a computed key, a
written binding, a returned member call, a read taken before a write, a longer
path, and an `undefined` claim no live completion hands back.

### The veto reads the member at return time

The member is the value the argument holds at that key **when the return reads
it**. The veto samples the parameter with an object holding a fresh token at
the key, and with `undefined`/`null` for an optional chain, and compares a
completion with the member as read after the call returns. Comparing with the
original token would contradict true claims: `callHandler`'s handler may call
`preventDefault()` before the return reads `defaultPrevented`. So
`p.key = 1; return p.key` certifies (its claim is true under this reading),
and a read taken before the write, then returned, refuses.

### The consumer

A member return and `undefined` name no leaf: nothing at the call site fixes
what the argument holds at return time, because the body or a callback it
invokes may have replaced it. Alone or in a union they read as no reactive
return, never as the whole argument. Before this ADR the projection read any
`parameter` output as the whole argument, path or not; that would have
invented a return, and no shipped contract relied on it (no snapshot moved).
This is the hiding direction and it is deliberate:
`readKey({ key: count })()` reports nothing, while `passThrough(count)()`
reports `SC1001`.

## Consequences

- Handshake protocol 63 -> 64 and facts schema 43 -> 44: a rebuilt
  `bin/solid-typefacts` and new certification pins.
- Contract corpus: possible operations 579 -> 607, proof candidates
  1,678 -> 1,726. A member proposal replaces the `plain` one in
  `implementation-census-{creates, described-accessor, reads, member-callee}`
  and `uncensused-invoking-forms`; the 13 `creates` exports on defaulted or
  written bindings are withdrawn by the census. New fixtures:
  `implementation-census-member-returns` and
  `reactive-ir/package-member-returns-consumer`.
- The generator's walk requires the member's receiver to be an identifier, so
  `options.inner.key` is no longer proposed as `key`.
- Measured on the 2026-09-25 coverage census (release binary, pinned corpus):
  `callHandler` closes `returns`; clean callables 421 -> 533 (the campaign's
  prediction, exactly), some-uses 279 -> 167, every-import unchanged at 294,
  nothing lost.

Still open: a store's member returned this way is hidden from the consumer;
paths deeper than one segment; `composeEventHandlers` (a deferred-invocation
item and a returned-function shape).
