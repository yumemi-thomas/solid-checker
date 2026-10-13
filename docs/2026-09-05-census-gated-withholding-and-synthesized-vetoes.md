# Census-gated withholding and synthesized vetoes — 2026-09-05

ADR 0036 was written before implementation. It changes what happens to a
proposed closure candidate the transaction cannot decide, and it lets the
checker synthesize the mandatory veto for a candidate no hand recipe addresses.
A contradiction still refuses the row; nothing about receipts already issued or
about the controlled execution profiles moves.

## What changed

**A census refusal withholds the candidate.** When Type Facts verification
refuses a `DomainExhaustiveness` demand whose subject is a proposable closure
candidate — `UnsupportedDemand` or `FamilyOpen` — `certify_value_only` withdraws
that candidate with reason `census refused: ` plus the census's own text,
re-derives the plan through the same weakening recipe gating uses, and acquires
again. Every other Type Facts error refuses the row as before. The case-set
variant keeps its shared batch as the fast path and hands a plan that needs a
second pass to the per-plan loop.

**An incomplete veto withholds the candidate.** A gate whose runs end in an
error or a timeout — including a worker that never reports because a sample
call loops forever, which the harness now names per session
(`ProbeHarnessError::SessionTimeout`) instead of failing the launch — withdraws
its candidate with reason `veto did not complete: gate …` and re-plans. A
contradiction never withholds.

**Vetoes are synthesized.** For a proposable candidate the hand corpus does not
address, and whose export-value transcript stated a unique call signature, the
checker writes a recipe module derived from that signature: a finite argument
sample (one representative per primitive domain, every literal partition case, a
counting callable for a callable slot, `undefined` for an optional slot; at most
six tuples), every call wrapped, `sample-threw` recorded, and the domain's
contradiction observed — a non-`undefined` result for `returns: []` (exact), an
own-property addition to `globalThis` during the window for `creates: []` (the
hand corpora's convention, not an exact observation, and the entry's coverage
limitation says so). The merged corpus — hand entries verbatim, synthesized
entries marked `provenance: "synthesized"` — lives in a private `0700`
directory of the transaction, loads through the same `RecipeCorpus` path, and is
removed with the transaction. A hand recipe always wins for its claim id. The
transaction gates on the hand corpus, acquires and censuses, synthesizes for the
still-withheld candidates, re-gates on the merged corpus, acquires again, runs
the gates, finalizes.

**Two smaller fixes landed with it.** The generator now binds an anonymous
callable's walk verdicts through its single-identifier `const`/`let`
declarator, so arrow-bound exports propose closures (7 of 9 silent corpus
exports now do; `open-dynamic-import-attribution`'s `loadLater` and
`published-export-entity`'s `runtimeArrow` stay silent and are recorded). And
the ecosystem runner reports withheld closures by reason
(`withheldClosureReasons`).

## The census fixture under ADR 0036

`the_census_certifies_a_generated_creates_candidate_and_withholds_its_siblings`
plans the generator's own document with one hand recipe (`plain`) and pins the
partition every other candidate reaches through a synthesized veto:

| domain | closes | withheld by the census | withheld by an incomplete veto |
| --- | --- | --- | --- |
| `creates` | `cycle`, `memberParameterRooted`, `noRecipe`, `plain`, `spreadArgs`, `switchBreak`, `toStringTagViaCall`, `viaHelperChain`, `whileBreak` (9) | 14 exports, each with the census's reason | `loopCall` |
| `returns` | `cycle`, `deep`, `setterOnParameter`, `stdlibRefInvoker`, `switchBreak`, `viaHelperChain`, `whileBreak` (7) | `labelledBreak` | `loopCall` |

`loopCall`'s body is `while (el) { mount(el); }`: every truthy sample loops
forever, the worker never reports, and the candidate is withheld — where the
first run of the suite had refused the whole row on a launch-level timeout.
`deep` closes `returns` while its `creates` is refused at the depth bound,
because the `returns` census reads the export's own completions and recurses
into no callee. Every `the_probe_gate_tracer_census_refuses_*` test now asserts
the withholding and its reason instead of a refused row; the contradiction test
is unchanged; `noRecipe` closes through a synthesized veto, and the no-harness
withholding stays pinned by the schedule test.

## Measurement

The ordinary three-row baseline, rerun against the rebuilt checker with the
checked recipe corpus
(`three-row-adr0036/after.json`, SHA-256
`5d72f0dc52d339b4805d877240d3f22b37e45428f44df6a9dc839b442d58de9e`). Before
this change one of the three rows certified; now all three do.

| row | before | after | withheld (reason) |
| --- | --- | --- | ---: |
| `@kobalte/utils@0.9.2\|solid1\|only` | refused: a hand recipe's gate did not complete | certified; 33 `creates` and 5 `returns` proposed closures pass | 1 (veto did not complete) |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | certified; 2 `creates` closures, 11 withheld for want of a recipe | certified; all 13 `creates` closures pass, the 11 through synthesized vetoes | 0 |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | refused: the census could not decide one accessor form | certified; 2 `creates` closures pass | 6 (5 census refused, 1 no call signature to synthesize from) |

Certification time per row stayed in the 20–27 s band the earlier baselines
recorded, synthesis passes included. `exportsProven` stays 0 on every row:
seven domains remain open on every real export, and this change decides none
of them.

## Checks run

- Rust: `solid-facts-backend`, `solid-reactive-ir` and `typefacts` library
  suites armed with the build's own pins; the four process suites; workspace
  Clippy with `-D warnings`; `cargo fmt --check`.
- Gates: contract corpus non-updating (95 fixtures, stable after the
  arrow-binding snapshot review), coverage (94 projects, 547 findings, no
  movement), the ecosystem-benchmark script tests (the runner's new
  `withheldClosureReasons` field), `git diff --check`.
- One pitfall hit twice in this work is worth naming: a test binary or a
  debug checker built without the certification pins is a different, weaker
  binary. The pin-guard tests caught the first (a shell that did not split the
  pin variables); the three-row run caught the second (a bare
  `cargo test --test …` rebuilt the debug binary). Both were rebuilt through the
  Makefile and rerun.

`make verify`, `make ecosystem-benchmark` and every baseline repin were
deliberately not run for this change.
