# ADR 0237: An open callback slot does not defer its argument

- Status: accepted and implemented (2026-10-08).
- Owner: `allowed_callback_spans` (`solid-reactive-ir/src/execution_role.rs`).
- Fixture: `fixtures/reactive-ir/package-open-callbacks-eager-argument`.
- Investigation: `rust/target/research/merged-props-swallow/FINDINGS.md`.

## Context

When an accepted contract leaves `callbacks` open, or states a slot as
deferred, the checker gives up on the timing of code that the export may run
later. It did that by adding the **whole argument expression** to the
deferred-callback regions. A read in such a region is a `DeferredCallback`
role, and `strict-read-untracked` never proves a violation there.

But only a function the export *receives* can run later. Everything else in the
argument is evaluated by the caller before the call. `combineProps(props,
...overlays())` in a component body reads `overlays` untracked right away.
Solid 2 warns about it, and the checker reported it until batch 3c gave
`combineProps` a contract with open `callbacks` (ADR 0236). Then the finding
disappeared, which is the one finding the 38-app sweep lost.

## Decision

For the slots a contract supplies, whether `deferred` or open, a region is:

- the whole argument, when the argument's runtime value (under TypeScript
  wrappers) is a function literal; otherwise
- the span of every function written inside the argument, and nothing else.

Slots that come from the dialect's own primitives are unchanged. Their
callback execution is proven per call by `callback_execution_at_call`.

The SC9005 obligation for the open claim is unchanged. A function delivered
through a call result, a spread, or an identifier remains open there.

## Consequences

- An eager read in such an argument now gets the caller's role. In a
  component body it is an SC1001 violation.
- Narrowing to function bodies is conservative, not exact. The body of an IIFE
  argument, `(() => overlay())()`, runs eagerly, but it is still shielded. That
  is an under-report, and no false positive can come from it.
- Every consumer of the regions (`static_rules`, `local_access`, `interproc`,
  `server_rules`, `static_api`) gets the narrower regions from the one
  constructor.
