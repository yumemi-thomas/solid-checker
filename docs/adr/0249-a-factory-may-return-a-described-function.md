# ADR 0249: A factory may return a described function

- Status: accepted and implemented (2026-10-08).
- Owners: `ValueShape::ReturnedCallable` and its validation, digest (new
  tags; existing digests unchanged) and certification refusal; the wire
  shape `returned-callable` (`contract_document.rs`,
  `schema/solid-reactivity.schema.json`); projection and binding in
  `contracts.rs` and `indexes.rs`; `returned_callable_obligations` in
  `local_access.rs`; `returnedClosures` in `scripts/author-contracts.mjs`.
- Fixture: `fixtures/reactive-ir/package-returned-callable-consumer`.
- Relation: extends ADR 0235 (a returned member's call graph) from members
  to a whole returned function and its own members. Designed in
  `rust/target/research/format-extension-2/`, patch drafted in
  `rust/target/research/returned-callable/`.

## Context

A whole returned function reopened the factory's `returns` (ADRs 0233-0234).
In the rc.13 corpus this was the most common reason a primitive import stayed
uncertifiable: `debounce`, `throttle` and `makeTimer` return functions, and
`debounce`/`throttle`'s result has a callable `.clear`.

## Decision

1. **A new value shape, `returned-callable`**, placed only as the whole
   output of a factory return.
   - It may have a call graph, using ADR 0235's normalization, and named
     members of any existing member shape.
   - An omitted graph is not an empty one.
   - It is authored-only: generation never proposes it, and certification
     refuses it.
   - It needs `returnedClosures` citations.
   - Captures, meaning graph parameters that name factory arguments, are not
     in this slice.
2. **Binding.**
   - A closed return enumerating it closes the factory's `returns`.
   - A `const` binding of the factory call binds the graph to that exact
     binding.
   - A direct call instantiates the graph, attributed to the caller (the
     origin ends in `]`, as in ADR 0235).
   - `f.member()` uses the member's shape.
3. **Fail closed everywhere else.** Each of these is a
   `reactive-dispatch-unresolved` obligation:
   - no graph;
   - disagreeing return branches;
   - a mutable binding or a mutated member;
   - a computed member;
   - passing, returning, exporting, aliasing or storing the function,
     including `[f]` and `{ f }`;
   - a JSX attribute value;
   - a call whose callee does not resolve to the exact binding, such as
     `(f as T)()`.

   A reference TypeFacts emitted no entity for, spelled like the binding in
   its scope, is also an obligation. The spelling only ever adds one.
   ADR 0234's exemption for an `on*` handler value stands. A discarded factory
   result needs nothing.

## Consequences

- `debounce`, `throttle` (with `.clear`) and `makeTimer` ship. Their probe
  pairs, including new cancellation pairs, pass in Chrome on rc.13.
- rc.13 corpus: primitive import declarations still raising SC9005 drop from
  27 to 24 of 43. No violation is added or lost.
- Browser ledger unchanged at 73 of 114. No correct twin has a violation.
- The drafted patch failed open in two places that review caught before
  landing: an array element or shorthand property holding the function, and a
  wrapped callee treated as a bound call. Both are now obligations, pinned in
  the fixture.
