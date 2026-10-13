# ADR 0278: Exact callback mapping and never-returning guards

- Status: accepted and implemented (2026-10-13).
- Owners:
  - `execution_role::project_callee_at_parameter`;
  - `SemanticLookup::never_returning_call_before` and
    `parameter_completion_calls` (`indexes.rs`);
  - `interproc::site_on_every_completion` and `cleanup::reached_on_every_call`.
- Fixes review round 2 for ADRs 0275–0277
  (`../solid-checker-rc13-corpus/experiments/review-0277/REVIEW-2.md`), at
  the owner's request (2026-10-13). No fact schema, contract or receipt
  change.

## Context

Round 2 confirmed five false violations and one silent result:
1. A guard that calls a `never`-returning function (`if (!ready) fail()`),
   when the caller catches the throw. ADR 0276 counted only a literal
   `throw` as an exit.
2. A helper that replaces its callback parameter before calling it
   (`cb = () => 0; cb()`).
3. A spread argument before the callback (`f(...xs, count)`).
4. `f.call(thisArg, …)`, read as a call of `f` with unshifted arguments.
5. An inline arrow passed to a `let` helper binding that may have been
   replaced (ADR 0221 was applied only to the by-reference path).
6. SC2001 going silent where ADR 0277 promised "an obligation, never
   silence".

## Decision

1. **A call TypeScript resolves to a `never`-returning signature is an exit.**
   A site written after one in the same function body is not a completion
   site, for reads, edges, callback parameters and the leaf walk. The set of
   such calls is hashed into `cross_file_proof_digest`, because the `never`
   lives in another file's types.
2. **A written parameter is not the caller's callback.** The completion
   table skips any parameter whose binding is written.
3. **The argument must provably reach the parameter.**
   `project_callee_at_parameter` refuses three cases:
   - a member callee whose own declaration is not the function, which
     refuses `f.call`, `f.apply` and `f.bind`;
   - a function value that is not current (ADR 0221);
   - an earlier spread argument.

   Every inline-role site and the by-reference row use it. An inexact
   by-reference mapping keeps an uncertifiable row.
4. **ADR 0277's "never silence" is narrowed.** SC1001, SC5001 and the leaf
   rules produce an obligation. SC2001 has no uncertifiable form for a
   consumer not proven to invoke the callback, and stays silent as it does
   for every unknown consumer.

## Consequences

- rc.13 corpus (browser host): 562 → 562 violations, 3,731 → 3,731
  uncertifiable, nothing moved; wall time 84 s.
- `asserts` functions are not exits yet. An assertion signature's return
  type is `void`, so the predicate needs a TypeFacts producer field: a new
  table version, a checker linkname for the type predicate, and a decoder
  change.
- An object-literal or class method callee (`bus.run(cb)`) still resolves
  exactly through its own declaration.

## Evidence

- New cases in `callback-parameter-completion`, all uncertifiable:
  `AfterFailArrow`, `ReassignedPassed`, `ReassignedArrow`, `SpreadBefore`,
  `CallMember` and `ReplacedRunner`.
- Also uncertifiable: `summary-completion-read` `CallsReadAfterFail` and
  `leaf-scope-helper-completion` `AfterFail`.
- `callee-callback-timing` `InvokedByMethod` and `host-callback-read-timing`
  `LocalBus` stay violations. A first version of rule 3 refused them.
