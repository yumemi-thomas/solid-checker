# ADR 0250: Opaque-member escapes are matched by the binder

- Status: accepted and implemented (2026-10-08).
- Owner: `LocalAccessContext::opaque_member_obligations`
  (`solid-reactive-ir/src/local_access.rs`).
- Fixture: `fixtures/reactive-ir/package-effectful-member-consumer`
  (`TupleInArray`, `TupleInShorthand`, `WrappedMemberCall`).
- Relation: the re-audit ADR 0249 called for. Its returned-function check
  failed open when a reference had no TypeFacts entity. ADR 0234's
  opaque-member check, which ADR 0235 extended, did the same.

## Context

A returned value holding an opaque or effectful member is tracked
member-by-member only when its single binding is used solely as a member
receiver. Otherwise the call is an obligation. That test, the alias test and
the member scan matched references by TypeFacts entity. An entity exists only
where the analysis demanded one. So a reference in an array element or a
shorthand property dropped out of the test, and a value escaping as
`keep([t])` looked member-only and stayed silent.

Separately, a call of a destructured effectful member through a TypeScript
wrapper, `(start as T)()`, was peeled to the name and treated as bound, but
its effects were never instantiated. Neither a finding nor an obligation
resulted.

## Decision

- References to the tracked binding are matched by the binder's declaration
  (`reference_declaration`), which every identifier reference has.
- A destructured member's described effects discharge only a call written on
  the name itself. A wrapped call is an obligation.

## Consequences

- The three fixture cases, previously silent, are obligations. No existing
  fixture finding moved.
- rc.13 sweep: no violation moves. Four new obligations appear in app-game,
  each a `createEventBus` result returned as a shorthand property and used
  elsewhere (`createEditorDocuments.ts:26` returned at `:361`;
  `navigation-popup.tsx:240`, `:296` and `:342`). They are real escapes the
  old check treated as clean.
- Browser ledger unchanged at 73 of 114. No correct twin has a violation.
