# Kobalte JS graph: two checker limitations removed

Date: 2026-09-04

ADRs 0011 and 0012 remove two independently reproduced preparation failures.
They do not certify Kobalte 0.9.2 yet. Missing `.d.ts` exports were not the
cause: declarations cannot prove the kind or behavior of a published runtime
value, and the relevant runtime/declaration bindings were already present.

## Measured dependency graph

Use the retained exact `@kobalte/utils@0.9.2` install, existing issuer and
authenticated registry cache. `/private/tmp/kobalte-092-frontier.mjs` calls the
real certifier with `--entrypoint . --dependency-graph-lane` and the checked-in
recipe corpus. Its injected fetch function throws on any cache miss. Every
recorded attempt asserts zero misses; no package installation is needed.

| Stage | Measured result |
| --- | --- |
| Before these changes | `solid-js@1.9.14 ./web` refuses `Aliases`, runtime kind `(Unknown, Unknown)` |
| Explicit unknown kind | Passes Aliases; catalog projection refuses the re-exported ErrorBoundary target |
| Same-package rebinding | Prepares 20 nodes, 13 archives, 25 acquisition units and 20 proposals; reaches native verification |
| Native verification | Refuses SetValues' unsupported positive read proof; no accepted graph |

The exact final refusal is:

```text
Type Facts demand sha256:71692cbcd0429cdf24d41397ef43268cac1c99e21208f322eb5763e88dd9c18c is locally open: operation-reachability (artifact-case:5531a12f062c3368613be1931afd1c4dc0bc5082f92aee0b11979d7e3bf98a35:SetValues): parameter-rooted read has no exact implementation call or use
```

This belongs to `@solid-primitives/keyed@1.5.3`. Its implementation combines
direct parameter reads with reads inside callbacks supplied to `mapArray` and
`createMemo`. The verifier requires exact source, reachability and execution
evidence. A captured read alone is insufficient. No change to that rule was
made here; the failed demand is retained, not silently removed after planning.

Artifacts are under the task's requested probe-ts scratch directory, in
`092-frontier/`: `baseline.audit.json`, `unknown-shape.audit.json`,
`identified-node.audit.json`, and `self-target.audit.json`.
The final quiescent replay after verification, `verified.audit.json`, reproduces
the same demand/refusal and graph counts with zero cache misses. Its SHA-256 is
`6bebffea72e306d75977b864327965a45819cb5340bcd16dc77a5175dcebdd0f`.

## What is accepted, and what remains unknown

An unresolved but present kind answer emits the existing `shape: "unknown"`
and wholly open behavior. The consumer can see exactly which export lacks
knowledge. Missing answers still refuse; a forged plain shape or closed creates
domain still fails native verification. An independent known export still
requires its complete proof and any scheduled veto. The fixture demonstrates
a real completed JS gate beside two unknown exports.

The catalog rebinder now recognizes an exact self-package semantic edge. The
authority-bearing native planner still needs the independent dependency plan
and replays export bindings against archive bytes. Both absence of that plan
and mutation of its target digest refuse in the new native fixture.

No transformer or byte substitution was introduced. The veto still observes
the exact published runtime bytes for its selected case, under sandbox scheme
6 and the existing harness/Node pins. No previously closed claim was weakened;
the newly retained unknown exports were formerly whole-case refusals.

## Snapshot review

The non-updating corpus identified three affected fixtures. Inspection of all
88 generated results established the complete change before `--update`:

- `class-expression-kind`: adds `./unresolvable` with unknown shape.
- `published-export-entity`: adds `./mixed` and `./unknown` with unknown shape;
  its now-empty refusal snapshot is removed.
- `non-emitting-module-target-control`: adds `./default-export` with unknown
  shape; it remains an ordinary runtime case, never an inapplicable type module.

Each new export has an empty call object, no operations and no closure
candidates. All earlier export claims remain identical. These additions expose
44 local open claims; they add no positive or negative proof. Main semantic
digests change accordingly. Native fixtures have no new main snapshots, so
`stableMainDocuments` remains 179. No benchmark or phase20/21 ledger is repinned.

## Three-row comparison and gate outcomes

The original targeted runner was repeated with the debug checker, the real Type
Facts binary, the same six-recipe corpus and `--keep-temp`. Compare this slice's
`open-kinds-verified.json` to `declaration-verified.json`, not the checked-in
benchmark that omitted recipes. Their SHA-256 values are respectively
`e8d38945a340228e04f51fec0f57af351772a4e3cabfbd9f079bdec14748bead` and
`cc538aa58b7731f172ae99632d33781b40774875c9269ec0c678735f6239978a`.

| Row | Candidates before → after | Gate outcome before → after |
| --- | ---: | --- |
| Kobalte 0.9.2 | 33 → 33 | noop incomplete → incomplete; row refused |
| Kobalte alpha | 13 → 13 | both clamp gates complete → complete; creates certified, 11 candidates withheld |
| i18n | 3 → 3 | census refuses → refuses; all three gates unexecuted |

The reused-proposal benchmark and explicit dependency-graph measurement are
different lanes. Improving the latter does not change the former's source
gate. The exact benchmark refusal remains:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

Every addressed gate, with unchanged before/after disposition:

| Export/case | Gate SHA-256 | Outcome |
| --- | --- | --- |
| noop | `a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc` | incomplete |
| clamp/import | `ce06289e646745cd9cf2ba8db28a8989e01ce17a9ada4107ade240067a4b3a7f` | completed, no contradiction |
| clamp/solid | `e066267de9781e7a38839c174e5354e02b76c999149160e9db32e135bf93387f` | completed, no contradiction |
| chainedTranslator | `00b9ed80e49fba15d38aab0b1e9f24f12ee2611633dc03eb510839c53ce14b75` | census refusal; unexecuted |
| flatten | `db644a8718275f606e5ecf0661e4c7666df8fec8521c71a352fbb22ccb91a45f` | same transaction refuses; unexecuted |
| scopedTranslator | `b56c77a2d09debd3875a393c453cdc715c07cd4003767bca41108c12c17ff2c4` | same transaction refuses; unexecuted |

Total candidates remain **49**, of which **9** select loadable JS cases.
**0/43 original candidates became probeable** in this or the preceding JS-case
work; the six additional JS candidates belong to independent cases. Exactly
**2 gates complete**, **0 contradict**, and **0 additional real closures** are
accepted by these two fixes. `exportsProven` remains **0** on every row.
No real contradiction was found to quote; contradiction regression tests pass.
The alpha receipts retain both accepted clamp main digests and probe roots from
ADR 0010. An incomplete audit's zero withheld count is not evidence that the
other 32 Kobalte source candidates ran.

## Verification

Cargo commands ran serially through the Makefile, with certification pins and
the local Type Facts binary armed. No `make verify` was run.

| Check | Result |
| --- | --- |
| Pinned debug build and probe harness | passed; 97 harness tests |
| Backend library | 367 passed |
| IR library | 234 passed |
| Contracts / diagnostics / dialects process tests | 7 / 15 / 37 passed |
| Contract corpus, final non-updating comparison | 88 fixtures passed |
| Coverage | 94 projects, 546 findings; unchanged |
| Ownership gate | 289 cases, 465 ledger rows, zero pending |
| Scripts Vitest suite, including phase19 | 153 passed |
| CLI tests and TypeScript type tests | 173 tests passed; type tests passed |
| Rustfmt then fmt check; workspace Clippy `-D warnings` | passed; pins rebuilt afterward |
| Schema parse, dialect manifests, whitespace checks | passed |

The native unknown-kind and same-package fixtures passed both their focused
runs and the full backend suite. The three reviewed corpus changes are the
only snapshot moves. Benchmarks and phase20/21 ledgers remain untouched.

## Next bounded work

The highest-priority Kobalte 0.9.2 path is exact read execution/provenance for
the SetValues demand, or a separately designed graph projection that avoids
asserting unused dependency-export behavior while retaining executable module
closure. Neither may waive a scheduled failed demand. Kobalte alpha's remaining
four JS creates candidates separately need accessor/iteration census evidence;
their exact refusals are in `2026-09-04-kobalte-remaining-js-candidates.md`.
The original 40 TypeScript source cases still require the ADR 0009 disposition;
adding declarations alone does not make Node execute those source bytes.
