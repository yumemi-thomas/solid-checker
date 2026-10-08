# ADR 0256: Returned graphs may capture factory values

- Status: accepted and implemented (2026-10-08).
- Owners:
  - the optional `captures` catalogue on a returned-callable graph and
    `ValueSource::Capture` (model, validation, digest family, certification
    refusal, wire, schema);
  - per-instance capture binding in the consumer (`contracts.rs`,
    `local_access.rs`, `interproc.rs`, `indexes.rs`);
  - `scripts/lib/capture-contracts.mjs`;
  - the `scheduled` spec;
  - `docs/package-contract-v2/returned-captures.md`.
- Fixture: `fixtures/reactive-ir/package-captured-callable-consumer`.
- Relation: census-2 item B-captures, extending ADR 0249. Drafted in
  `rust/target/research/captures/` over three rounds.

## Decision

1. **A returned graph may name captured factory values.** A capture is an
   exact factory argument, resource or operation result, retained at factory
   call time. The graph refers to it with `from: {capture: id}`. Ordinary
   parameters still mean the later invocation's arguments.
   - It is authored only, refused by certification, and needs
     `captureClosures` citations.
   - Recursive and member-class captures, wildcard paths, and capturing the
     enclosing return are refused.
2. **The consumer binds captures per returned instance** from the exact
   immutable factory arguments. A mandatory inline invocation of a captured
   accessor is a read at the call of the returned function, so
   `const f = createTicker(read); f()` in a component body is a proven
   SC1001. A captured function literal runs at that call. Building the
   literal reads nothing.
3. **Fail closed.** Each of these is a `reactive-dispatch-unresolved`
   obligation:
   - a captured or returned value that escapes (arrays, shorthand, aliases,
     casts, passing, returning, exporting, JSX values, and `on*` values for
     this vocabulary);
   - a mutable binding;
   - an unknown capture identity;
   - a captured arrow with parameters.
4. **Review found two defects before landing.**
   - Round 2 silently dropped the read of a captured accessor. An
     argumentless call demanded no resolved-call fact, and capture reads were
     marked summary-attributed. Round 3 fixed both.
   - The authoring tool ran the capture check on nested returned graphs and
     refused their legitimate captures. It now runs once, from the factory
     claim.
5. **`debounce` and `throttle`** state that calling the returned function
   invokes the captured callback. All scheduled probe pairs pass in Chrome on
   rc.13.

## Consequences

- Primitives ledger unchanged at 92 of 112. Captures alone completes no
  case: `createScheduled` also needs returned state, and `translator` also
  needs dictionaries. No correct twin has a violation.
- rc.13 corpus: no violation added or lost; uncertifiable +8. The added
  findings are debounced or throttled calls whose callbacks take parameters.
  Before this, those calls produced no finding at all, although ADR 0249's
  graph left `callbacks` open.
