# ADR 0146: A described callable may read a signal its export created

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the semantic model (`DescribedRead::OwnedSignal`,
  `ValueShape::ReadValue`), the dialect (`Dialect::inert_accessor_read`,
  `unambiguous_inert_accessor_read`), the Type Facts producer
  (`ImplementationValueSource::arguments_primitive_syntax`, `ReturnSite::call`;
  handshake protocol 66, with ADR 0145), the generator's reading walk
  (`returns_walk::reading_callable_returns`, `inferred_contract.rs`), the
  described-callable census and veto (`type_facts.rs`,
  `synthesized_vetoes.rs`), and the consumer's projection (`contracts.rs`)
- Relation: fills the one nested domain ADR 0145 left empty. The owner's
  decision of 2026-09-28 names this case: "a captured signal/accessor makes
  the nested `reads` state it", and "a returned accessor stays an accessor".

## Context

After ADR 0145 a returned literal that reads anything is withdrawn, and so is
every export whose reactive analysis describes the return as an accessor: the
generator published `Reactive { accessor }`, which no census decides. The
owner's example is `createMediaQuery`: calling what it returns in a component
body is an untracked reactive read, and the consumer should say so.

## Decision

### The claims

- `reads` may name `owned-signal`: one invocation of the described callable
  observes the current value of a signal **the export's own invocation
  created** with the dialect's `createSignal`, on the invoking caller's stack
  and in that caller's tracking context, and the read runs no code.
- `returns` may name `read-value`: exactly the value such a read observed,
  handed back unchanged. Valid only inside a described callable that reads.

`createCounter` (`const [count] = createSignal(0); return count;`) and
`() => count()` both publish `reads: [owned-signal]`, `returns: [read-value]`;
`() => count() * 2` publishes `returns: [plain]`.

### Why the read is inert, and when

The dialect states it (`Dialect::inert_accessor_read`), per audited release:
`@solidjs/signals@2.0.0-rc.9` `createSignal(e, t)` takes the writable-memo path
only when `typeof e === "function"`; otherwise it builds a plain `signal` node,
whose only callbacks are the options' `equals` (called by the setter) and
`unobserved` (called when the last observer unlinks). Its accessor is
`read.bind(null, node)`, and `read` of a node with no compute function, no
firewall owner and no pending status serves the value, linking the observer
when tracking; it runs no code (`dist/prod/signals.js` `createSignal`,
`dist/prod/core/core.js` `read`; the same shape in `dist/dev.js` and in rc.3 and
rc.6). So the row's precondition is that **every argument of the creating call
is a primitive by grammar**: no function first argument, no options object.
`createMemo`, `createOptimistic` and every other primitive are not stated.

### The census

A value is an inert owned accessor (`inert_owned_accessor_witness`) exactly
when its rooted trace is a call result of a name every dialect exporting it
states inert at that slot, every argument of the traced call is a primitive by
grammar (`argumentsPrimitiveSyntax`, protocol 66), and every call of that
target in **the export's own implementation** resolves to a declaration
inside an authenticated dependency snapshot that is an audited dialect archive
-- never the certified artifact. A signal created at module scope, in a helper,
or over the caller's argument (`createFrom(initial)`) is unproved.

- A **returned accessor** (the site's whole value, no literal) is described by
  its own `sources` trace: `reads: [owned-signal]`, `returns: [read-value]`.
- Inside a **returned literal**, ADR 0145's walk now admits a call, at depth 0
  and not inside a nested callable, whose callee traces to such an accessor, as
  the disposition `owned-signal-read`. The literal states `owned-signal` when
  it has one; a completion is `read-value` when its expression is exactly one
  of those calls (`ReturnSite::call`, protocol 66), `plain` when primitive by
  type and grammar, and anything else refuses.

The veto is ADR 0145's module; a read is not observed, and a `read-value`
return leaves the nested completion unchecked.

### The generator

Where the reactive analysis describes the return as an accessor, and every
value-carrying completion is an identifier or a function literal (or a
conditional of them), the generator proposes the reading shapes: an identifier
is the accessor itself; a literal's call completions propose `read-value` and
its others `plain`. The census decides.

### The consumer

A described callable that reads projects to the `accessor` leaf (ADR 0145's
projection), so `const count = createCounter(); count()` in a component body is
the untracked read `SC1001` reports, and the same read inside JSX is tracked
and clean (`fixtures/reactive-ir/package-described-callable-consumer`, `tsc
--noEmit` clean).

## Consequences

- The census premises are pinned on synthesized transcripts
  (`an_owned_signal_is_witnessed_only_for_an_inert_audited_accessor`: seven
  refusals by name), the producer facts in
  `TestCallResultSourcesStateTheirArgumentsAndReturnsTheirCall`, the dialect row
  in `only_the_plain_signal_accessor_read_is_inert`, and generation in
  `fixtures/package-contracts/implementation-census-owned-signal-reads`.
- No end-to-end certification in this repository reaches it: the native
  tracer harness cannot stand up an audited `solid-js` archive (its integrity is
  the published tarball's), so the positive path is exercised only by the
  certification metric over real packages.
- Five exports in two other corpus fixtures that returned an accessor now
  propose the reading shape instead of the undecidable `reactive` output
  (`callback-slot-derived-store`, `composed-operation-provenance`).

## What still refuses

- A signal the export did not create in its own body, or created over the
  caller's value or with options; `createMemo` and `createOptimistic`
  accessors; a store read; a read inside a nested callable of the literal.
- A dependency's signal-returning export (`createHydratableSignal`, which
  `createMediaQuery` uses): its accessor is described only when that
  dependency's accepted contract states its return exactly, which is a
  composition this ADR does not add.
