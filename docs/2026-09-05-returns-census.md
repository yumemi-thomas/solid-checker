# The `returns` census — 2026-09-05

ADR 0035 was written before implementation. It makes `returns` the second
behavioral call domain the implementation census of ADR 0008 decides, for the
empty closure only: `returns: [] closed` denies that one invocation yields a
value to its caller, and the semantic model's 2026-09-03 decision that a
valueless completion is not a `return` operation is what makes the question
control-flow-decidable.

## What the census decides

On the demanded export's own implementation transcript:

1. the completion form is `plain` — an `async` function, a generator and an
   async generator hand their caller a promise or an iterator whatever the body
   does, and an unclassified or unstated form refuses;
2. the control-flow census is present and every incompleteness row is
   `reachability-lower-bound`;
3. every return site at the `MayExecute` floor is bare, or carries a value the
   producer proved unreachable.

No callee is dispositioned and no local declaration is recursed into: a
helper's return value reaches the caller only through a return site of the
export's own, which premise 3 reads. A `returns` proposal that enumerates
operations is refused as a nonempty `creates` enumeration is.

## Producer

`ExportImplementationTranscript.completionForm` is stated on every transcript
that reached its implementation, from the `async` modifier and the asterisk
token of the declaration itself. The handshake protocol moves from 18 to 19 so a
verifier expecting the fact refuses a producer that cannot state it rather than
reading absence as plain.

## Generator

`ClaimDomain::PROPOSABLE` gains `Returns`. A dedicated valueless-completion
walk over the generator's syntax facts clears a block-bodied, non-`async`,
non-generator function whose own body carries no `return` with an expression; a
return strictly inside a nested function declaration, expression or arrow is
that callable's completion and is skipped. The walk found one trap during
implementation: a return fact's span is its argument's span, so `return () =>
…` yields a fact whose span *is* the arrow's, and a non-strict containment test
read the arrow as owning the return. Strict containment fixed it, and the review
below is what caught it.

The generator's existing `returns: Known(None)` means "no reactive return
described" — true of every export returning a plain value — and had been
normalizing to a closed `Complete([])` that weakened into a sidecar candidate. It
now normalizes to `Unknown` unless the walk is clean, so the candidate exists
only where the generator derived it.

## Corpus movement

Reviewed in full before the snapshot update was accepted:

| movement | count |
| --- | ---: |
| spurious `returns` closure candidates withdrawn from plan sidecars | 179 |
| `returns` closure candidates added | 0 |
| exports gaining `closed: ["creates", "returns"]` | 53, across 28 fixtures |
| exports with both a void and a value-returning artifact case | 1, closed in the void case only |

Every export that gained the closure was checked against its source for a
value-carrying return in its own body, an `async` modifier or a generator
asterisk; none had one. The corpus is stable on a non-updating rerun.

## Fixture

`implementation-census-returns` pins each premise end to end through the probe
gate tracer with a hand-closed `returns: []`:

| export | outcome |
| --- | --- |
| `bareCompletion`, `earlyBareReturn`, `bareReturnInLoop`, `nestedReturnsValue` | certify through `probe-recipes/valueless.mjs`, nonempty probe gate root |
| `returnsValue`, `expressionArrow` | refuse: `value-carrying completion`, reach `reachable` |
| `valueReturnInLoop` | refuse: `value-carrying completion`, reach `unknown` |
| `asyncVoid` | refuse: `async implementation` |
| `generatorVoid` | refuse: `generator implementation` |

The unreachable-value admission is pinned on a synthesized transcript by
`returns_census_admits_only_bare_or_unreachable_completions`, because the
generator's walk has no reachability and never proposes such an export. The
generator side is pinned by
`the_generated_returns_fixture_carries_its_valueless_candidates_into_planning`:
four `returns` candidates, nine `creates` candidates, thirteen gates. (The
`const` arrow export first exposed that the generator bound walk verdicts to
function declarations only — fixed the same day; see the backlog entry on
arrow-bound exports.)

## Measurement

The ordinary three-row baseline
(`three-row-adr0035/after.json`, SHA-256
`570be772a0533c2098a81c147eb74abbe03a65777076d55bafbd12e8dfd65b1a`) is
unchanged in class, reason and withheld-closure count:

| row | outcome | withheld closures | `exportsProven` |
| --- | --- | ---: | ---: |
| `@kobalte/utils@0.9.2\|solid1\|only` | mandatory probe gate `a9c9b71f…` did not complete | 0 | 0 |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | certified; existing JavaScript closures | 11 | 0 |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | independent accessor-census refusal | 0 | 0 |

No Kobalte alpha export proposes `returns: []`: the published JavaScript
exports the walk can bind all complete with a value. The domain's real-row
yield therefore waits on void exports with recipes, and `exportsProven` stays 0
because seven other domains remain open on every real export.

## Checks run

- Go producer suite, including the completion-form test.
- Rust: `solid-facts-backend`, `solid-reactive-ir` and `typefacts` library
  suites armed with the build's own pins; the four process suites; workspace
  Clippy with `-D warnings`; `cargo fmt --check`.
- Gates: contract corpus (reviewed update, then stable non-updating rerun),
  coverage (94 projects, 547 findings, no movement), dialect manifests, schema
  `jq`, `git diff --check`, the phase 19 audit (185 mains).

One trap was hit and corrected during the work: a bare `cargo test --test
contract_interface` rebuilt the debug checker without the certification pins,
and the first three-row run against that binary refused every row on producer
provenance. The binary was rebuilt through `make build-checker-debug` and the
measurement rerun.

`make verify`, `make ecosystem-benchmark` and every baseline repin were
deliberately not run for this change.
