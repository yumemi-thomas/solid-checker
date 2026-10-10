# ADR 0234: Calling an opaque returned member is a proof obligation

- Status: accepted and implemented (2026-10-08).
- Owners: `project_member_shape` and `OPAQUE_MEMBER` (`solid-reactive-ir/src/contracts.rs`);
  `LocalAccessContext::opaque_member_obligations` (`local_access.rs`);
  `validate_contract_return` (`lib.rs`).
- Relation: replaces ADR 0233's refusal with the consumer rule it was waiting
  for, and is the prerequisite for the returned-member format extension
  (`effectful-callable`, E1 in
  `rust/target/research/format-extension/DESIGN.md`). Amends ADR 0177: calling
  an `unknown` member no longer reports nothing.

## Context

A contract may return a tuple or object with a member it does not describe:
`callable` (callability only) or `unknown`. The consumer kept only reactive
leaves, so a call of such a member left no trace. While `returns` stayed open,
the import's `package-contract-incomplete` notice covered it; once `returns`
closed, the call was certified clean. `createRAF`'s `start` reads `running`
untracked, and Chrome warns about it in a component body (ADR 0233).

## Decision

1. **The projection keeps an opaque member.** A `callable` or `unknown` member
   of a returned tuple or object becomes an `opaque-callable` leaf. It names
   no reactive source. A whole-`callable` return is unchanged and still
   reopens `returns`.
2. **Using it is an obligation** (`reactive-dispatch-unresolved`,
   uncertifiable):
   - a call of a destructured opaque member in a component body, module scope
     or compiler callback;
   - any other use of it, except a JSX `on*` handler value: it may be called
     wherever it goes;
   - for a result bound to one name, every member access selecting an opaque
     member, unless it is called inside a nested non-component function;
   - a result holding an opaque member that is neither destructured nor
     discarded, unless every reference to its one name (no alias) is a direct
     member receiver.
3. **A closed `returns` over such a member is allowed again.** `createRAF` and
   its `default` alias close it as ADR 0227 did; `createDate` and
   `createDateNow` close it, citing their one browser return path. Closing
   enumerates the tuple only; it claims nothing about calling its members.

## Consequences

- Coverage: `partial-structural-return-consumer` gains the two member-call
  obligations (`stop()`, `panel.stop`) and two new cases: a member used as an
  `onClick` value stays clean, and an escaped return is an obligation. No
  other fixture moves.
- Primitives ledger, browser: 20 report correctly (was 16). The two
  `createRAF` top-level reads are back, and both date cases certify their
  correct twin clean while their misuse stays a violation.
  `raf-createRAF-start-top-level` is now an uncertifiable obligation where it
  was silent; it expects the violation Chrome shows. No correct twin on any
  host has a violation.
- Still open: what calling such a member does. A claim describing it
  (`effectful-callable`) would turn these obligations into proofs.
