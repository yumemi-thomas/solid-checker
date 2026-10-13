# ADR 0262: a callback that completes with a primitive

- Status: accepted and implemented (2026-10-09).
- Owners: `solid_facts::ast::primitive_completion`;
  `SemanticLookup::scalar_builtin_call_result`;
  `callback_result_has_primitive_completion` in `local_access.rs`.

A package contract can say a callback argument's result is called later only
if it is callable: an optional, callable-only Call use. `createPolled`'s
`poll-source` is one. When the argument was `() => Date.now()`, the result was
Unknown, and the use stayed an obligation, though a number can never be called.

The consumer now discharges such a use when the callback's result is provably
a runtime primitive. It applies only when the census is closed and contains
nothing but empty-path, `min: 0`, callable-only Call uses; it proves nothing
about the callback census, producer effects, tracking or ownership.

The callback must be one synchronous literal arrow with one return covering
every normal completion. That return is primitive when:

- syntax proves it: primitive literals and untagged templates; `typeof`,
  `void`, `!` and strict-equality results; numeric unary results; and
  arithmetic, bitwise or coercing comparisons whose operands are recursively
  primitive. Object coercion is never a premise, and casts and annotations
  never make an unknown value primitive.
- or it is a zero-argument `Date.now()` or `Math.abs`/`ceil`/`floor`/`round`.
  The call must be valid (never Recovery), single, not spread, constructed or
  optional, with a receiver that resolves exactly to the standard-library
  global. Visible writes, deletes or escapes of that global in the configured
  project veto the proof. By ECMAScript §21.4.3.1, `Date.now` returns a time
  value, which is a Number.

## Premise

This is the repository's reviewed standard-library premise (ADRs 0167, 0190,
0211): exact lib declarations, a write/delete/escape veto, and no model of
reflective or external patching. A lib declaration alone is not the premise,
and the name `Date` is never matched. `Date.now` reads ambient time; this ADR
does not make it pure or certifiable as such.

## Measured consequences

- Fixture `package-scalar-result-consumer`: the syntax and built-in cases are
  clean. The producer's own write is still an SC2001 violation. Annotated,
  cast, coerced, typed-local, computed, optional, unreviewed (`Math.max`),
  constructed, async, named, multi-return, `finally`-override, open-census and
  required-call shapes, plus shadowed or replaced `Date`/`Math`, keep the
  obligation.
- Primitives ledger, browser: 96 of 111 report correctly (was 95):
  `createPolled` at module scope. The `createPolled` top-level-read twins lose
  their dispatch obligation but keep `package-contract-incomplete` for
  callbacks. No correct twin has a violation.
- rc.13 corpus: no violation or uncertifiable site added or removed.
