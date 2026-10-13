# ADR 0281: A store member read inside an argument is no escape

- Status: accepted and implemented (2026-10-13).
- Owner: `source_discovery::member_chain_escapes`, used by
  `project_returned_source` (ADR 0222).
- Found while analyzing `@tanstack/solid-query` 6.0.0-rc.0 source as app code
  (dependency-source experiments, 2026-10-13); research report
  `rust/target/research/dep-mode/nested-throw.md`.

## Context

ADR 0222 proves that a hook returns the store it created, so a caller's read
of that store is a store read. The proof refuses a root that escapes or is
aliased. It implemented that by refusing any reference to the root written
inside any call argument.

`useMutation` reads `state.isError` inside a `createRenderEffect` compute, so
the proof failed. The returned value then had no source, and every caller's
read went silent, not even uncertifiable. The `throw` in that compute was
incidental: `createMemo(() => { const v = state.search; … })` fails the same
way.

## Decision

A reference to the root inside a call argument is an escape only when it can
hand the root, or a sub-proxy of it, to other code. Take the member chain
rooted at the reference (`state`, `state.items`, `state.items.length`). It
escapes when any of these holds:
- the chain is the bare root;
- the chain is itself a call argument;
- the chain is a callee, which receives the proxy as `this`;
- a function returns the chain, from a `return` or a concise arrow body.

A chain consumed where it is written is not an escape: a statement, a
declaration initializer, or an operand inside a callback the argument holds.
Every other ADR 0222 rule is unchanged.

## Consequences

- The solid-query shape and memo reads are proven again.
- Root and sub-proxy escapes stay refused. That covers `sink(state)`,
  `sink(() => state)`, `sink({ s: state })`, `sink(state.items)`,
  `createMemo(() => state.items)` and `state.items.indexOf(1)` inside a
  callback.
- No cache change. A hook file that returns a bare identifier is already
  bound into `cross_file_proof_digest` (ADR 0222).

## Evidence

Fixture `reactive-ir/returned-store-nested-read`: three proven reads and six
refusals; `tsc --noEmit` is clean.
