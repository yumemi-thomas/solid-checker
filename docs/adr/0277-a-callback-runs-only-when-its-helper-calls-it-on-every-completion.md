# ADR 0277: A callback runs only when its helper calls it on every completion

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `solid-reactive-ir/src/execution_role.rs`:
    `invokes_parameter_during_call` and `calls_parameter_in_body`;
  - `SemanticLookup::parameter_called_on_every_completion`, and its table in
    the cross-file proof digest (`indexes.rs`);
  - the accessor-by-reference row in `interproc.rs`.
- Follows ADRs 0275 and 0276 at the owner's request (2026-10-13). Fixes the
  two pre-existing false-violation paths that ADR 0276's review round 1
  confirmed. No fact schema, contract or receipt change.

## Context

A callback passed to a project helper was taken to run during the call
whenever the helper called that parameter somewhere written directly in its
body. Review round 1 for ADR 0276
(`../solid-checker-rc13-corpus/experiments/review-0276/REVIEW-1.md`) showed
false violations:
- `runWhen(false, count)` and `runWhen(false, () => count())`, where the
  helper calls `cb()` only under `if (flag)`;
- the same through a ternary, a loop over an empty list, and `??=`;
- an SC5001 error for an async memo read in such an arrow.

## Decision

1. **A helper invokes its parameter during the call only on its
   every-completion path.** `invokes_parameter_during_call` holds when a call
   of the parameter, by its symbol, is a completion site of the helper's
   synchronous body (ADR 0276's `site_on_every_completion`). Every consumer
   of that predicate inherits the rule:
   - the role of an inline arrow passed to a project helper;
   - the stored-literal and argument-invocation proofs;
   - the accessor-by-reference read.
2. **A call on some paths is an obligation.** (Narrowed by ADR 0278:
   SC2001 has no uncertifiable form and stays silent, as it does for any
   unknown consumer.) For an accessor
   passed by reference, a body call off the completion path
   (`calls_parameter_in_body`) still yields the read row, marked not
   performed by this call, which projects as uncertifiable. An inline arrow
   falls back to the existing "consumer not proven to invoke it" obligation.
3. **The answer is bound into result reuse.** Moving `cb()` into a branch
   changes no read summary, so the project-wide table of
   `(file, function, parameter)` completion calls is hashed into
   `cross_file_proof_digest`. A change there discards retained late-stage
   results.

## Consequences

- rc.13 corpus (browser host): 564 → 562 violations, 3,729 → 3,731
  uncertifiable, nothing added. Wall time is unchanged (84 s).
  - In app-game `createDragSensor`, the parameter is called only inside a JSX
    attribute, which the compiler defers to a getter.
  - In inferay `createDockLayout`, the parameter is called only in a nested
    arrow's default parameter (`target = input()`). The old body predicate
    counted that call as the helper's own.

  Neither was proven to run during the call.
- A call `cb?.()` is not a completion site, so an always-defined callback
  invoked optionally is an obligation. That proves less.
- Not yet: argument sensitivity (`runWhen(true, count)` stays uncertifiable),
  and conditional invocation through a member of a parameter.

## Evidence

- Fixture `reactive-ir/callback-parameter-completion`. It has two
  violations: by reference and inline, called on every completion. It has
  six uncertifiable results: by reference behind `if` and a ternary, inline
  behind `if`, a loop and `??=`, and the SC5001 async case. `tsc --noEmit`
  is clean.
