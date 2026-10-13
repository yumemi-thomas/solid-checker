# ADR 0279: Assertion and never signatures, and exact method callees

- Status: accepted and implemented (2026-10-13).
- Owners:
  - TypeFacts producer: `Call.AssertionSignature` and `Call.NeverReturns`
    (`apps/solid-typefacts/internal/typefacts`, table schema v19), read
    through the `getTypePredicateOfSignature` linkname and tsgolint's
    `TypePredicate_kind` mirror in `shims/checker`;
  - Rust client: `ResolvedCall::assertion_signature` and `never_returns`
    (`rust/crates/typefacts`);
  - `SemanticLookup::never_returning_call_before`;
  - `execution_role::member_runs_declared_method`.
- Follows ADR 0278 and review round 3
  (`../solid-checker-rc13-corpus/experiments/review-0278/REVIEW-3.md`), at
  the owner's request (2026-10-13).

## Context

ADR 0278 made a `never`-returning guard call an exit, so a caller that
catches its throw does not see a later read or registration as performed. It
left two gaps:
- `asserts` functions (`assertReady(ready)`), whose return type is `void`;
- the guard's `never`, which it read off `returnTypeText`, which the facts
  reference defines as display metadata.

Review round 3 also found two more false violations:
- a method call trusted to run the declared method body when the receiver
  may hold an override or a patched member;
- a `never` call in a parameter default, which was not counted.

## Decision

1. **The producer states both signature facts.** Table schema v19 appends a
   flag word to every resolved call:
   - bit 0: the selected signature carries an `asserts` type predicate;
   - bit 1: its return type is the `never` type, by the compiler's type
     flag.

   No other row changes. Both sides pin v19.
2. **Either flag makes the call a possible exit.** A site after such a call,
   in the function's body or parameter list and not in a nested function, is
   not a completion site. This covers reads, edges, callback parameters and
   the leaf walk.
3. **A method callee must run the declared method.** The member must be one
   of these:
   - a module namespace member;
   - a member no assignment anywhere writes by name, whose receiver is a
     `const` bound to the object literal holding the method;
   - such a member whose receiver is a `const` bound to `new C()` of the
     exact class that declares it.

   Otherwise the mapping is inexact and the result is an obligation.

## Consequences

- rc.13 corpus (browser host): 562 → 562 violations, 3,731 → 3,731
  uncertifiable, nothing moved; wall time 85 s.
- The TypeFacts binary was rebuilt (`make build-typefacts`). A producer and
  client of different table schemas refuse each other at handshake.
- Recall: an exhaustiveness guard (`if (impossible) assertNever(kind)`)
  before a read now leaves that read uncertifiable.
- Not yet:
  - `fail!(…)`, `(fail as F)(…)` and `fail?.(…)` carry no resolved call at a
    span the checker reads, so those forms are not exits;
  - a helper that throws without a `never` or `asserts` type
    (`function check(ok) { if (!ok) throw … }`, inferred `void`) is not an
    exit either, because the type system does not record the throw.

## Premise: a call completes normally unless its type says otherwise

Owner decision, 2026-10-13, option (b) of review round 3. A call is assumed
to return normally unless its selected signature is `asserts` or returns
`never`. A literal `throw` statement written in the walked body is an exit
(ADR 0276). This extends ADR 0274's premise, which already puts an earlier
getter's abrupt completion out of scope.

A helper that may throw without declaring it, when a caller catches the
throw, is outside the model. Its later reads and registrations are taken to
run. The alternative was to treat every helper call inside a caller's `try`
as unproven, which the owner declined for its recall cost.

## Evidence

- Go: `TestResolvedCallFlagsAssertionAndNeverSignatures` covers `never`,
  `never` through an alias, `asserts x`, `asserts x is T`, an assertion
  method, a type guard and `void`.
- Fixture cases:
  - uncertifiable: `summary-completion-read` `CallsReadAfterAssert` and
    `CallsReadAfterFailingDefault`, `leaf-scope-helper-completion`
    `AfterAssert`, and `callback-parameter-completion` `MaybeOverride`;
  - still violations: `BusMethod`, `ExactInstance` and
    `callee-callback-timing` `InvokedThroughNamespace`.
