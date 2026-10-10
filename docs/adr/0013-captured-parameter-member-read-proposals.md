# 0013 — Retain uncertainty when a parameter-member read is captured

Status: accepted; implemented
Date: 2026-09-04

## Decision before code

Preserve whether a parameter-member invocation belongs to its summary owner's
own function body. If any contributing member invocation belongs to a nested
callable, keep the generated export's reads domain open. The compact inference
model currently cannot represent that read's execution chain, trigger, tracking
or schedule. Do not emit a direct, untracked, same-stack read operation from
that incomplete summary. Direct-body member calls retain their current claims
and ordinary native verification. Missing function ownership is uncertainty.

This changes proposal generation, not proof acceptance. It removes an
unsupported positive assertion before demand planning, visibly retaining an
unknown reads domain. No failed demand is waived, no reads domain is closed to
empty, and no creates candidate or mandatory veto is removed. Since the compact
read summary is all-known or open, a mixed direct/captured export conservatively
withholds that whole domain; partial positive read knowledge needs a richer
representation in a separate change. Independently known domains remain.

## Evidence

The previous final Kobalte 0.9.2 replay is a reproducible baseline. A temporary,
targeted diagnostic on its failed read demand reports parameter 0, exact path
`of.values`, lower bound zero. Its matching invocation is captured in the
arrow supplied to `mapArray`; the uncaptured use of `props.children` has the
wrong path. The diagnostic was removed immediately after the one run.

The reduced fixture `captured-parameter-member-read` contains no Solid import:
`retain(() => props.of.values())`, where retain only returns its argument and
the caller discards it. Before the change the generator emits a same-stack
read of `of.values` even though that arrow never runs. The direct sibling emits
the same operation. The mixed sibling collapses distinct direct/captured paths
to a root-parameter read. This demonstrates loss of execution provenance,
independently of Kobalte's callback semantics or declaration types.

## Alternatives and proof boundary

Accept any captured call: rejected; the retained-but-never-called fixture is a
counterexample. Reuse the verifier's general callback execution walk: rejected
for this read claim, since that walk also admits returned and deferred
callbacks and does not establish same-stack, untracked execution. Infer that
`mapArray` and `createMemo` always execute the desired path: rejected without
an exact result-flow and dialect execution proof. A future richer claim can
bind that chain, but a parameter path alone is not such evidence.

Keeping the whole graph refusal is sound, but the unsupported read originated
in our proposal. Recording unknown knowledge is more precise than proposing an
operation whose execution context the generator discarded. Consumers can see
the open reads domain and cannot use it as either positive or negative proof.
The native verifier remains unchanged and must still refuse the old forged
direct-read claim. Exact bytes, snapshot replay, harness/Node pins, sandbox
scheme 6 and every scheduled veto retain their existing meaning.

## Required verification

Pin captured, direct, mixed and unestablished ownership; retain execution
provenance in incremental cache identities. Review corpus differences before
updating snapshots. Replay the original offline dependency graph and report
the next refusal or acceptance exactly. A graph advancing is not itself a
certified export or a completed probe gate.

## Implementation and measured graph result

`ParameterMemberInvocation` now retains `in_owner_body` beside the exact
parameter index and path. The producer of this internal IR record compares the
innermost AST function body with the summary owner's body; absent ownership is
false. Graph assembly, cached dependency identity and contract projection retain
the field. Contract projection emits unknown reads when any contributing
invocation lacks that direct-body context. No native acceptance rule changed.

The new corpus fixture emits no read operation for `captured` or `mixed`, while
`direct` keeps its exact parameter-0 `of.values` read. Its native test verifies
that evidence, then transplants the direct claim onto `captured` and observes
the original exact refusal. The IR cache regression changes only execution
context and requires the former result to invalidate. Both focused checks pass.

The same offline graph advances past SetValues. Comparing the retained demand
plans removes exactly two proposed read operations, `SetValues:read-0` and
`MapEntries:read-0`; seven other read operations remain. No new read operation
is asserted. The next first refusal is:

```text
Type Facts demand sha256:05596de8b9acb119628dd17fb20788932d6664ecff0c36c3fbdb3f6a710b143f is locally open: callable-path (artifact-case:5d5e36f1a70c0df7d4fd05f5c56a1db35ee73b3c7399caa64ce19654a004f2e5:createReaction): callback parameter has no exact direct-call or resolved-argument flow
```

It belongs to `solid-js@1.9.14`'s root. The graph still prepares 20 canonical
nodes from 13 authenticated artifacts with zero cache misses. This removes a
false proposal; it does not yet yield an accepted Kobalte 0.9.2 graph.

Scratch evidence is in the task's `probe-ts/092-frontier/` directory:
`verified.audit.json` is the previous slice's uninstrumented baseline and
`captured-read.audit.json` is this slice's first replay. The temporary diagnostic
is retained only in scratch; its source instrumentation is completely removed.

## Snapshot review

The non-updating corpus gate first refused the new fixture's missing snapshot.
A complete inspection run retained all 89 generated outputs. All 88 existing
fixtures remained identical; the new main contains callable shapes, one direct
read operation and unknown captured/mixed reads. Only then did `--update`
write the new main, proposal and refusal sidecars. The fixture was staged before
phase19, and its stable main advances `stableMainDocuments` from 179 to 180.

## Three-row measurement and final verification

The targeted runner uses the same six-recipe corpus as the previous slice;
the checked-in benchmark without recipes is not the baseline. Before:
`probe-ts/open-kinds-verified.json`, SHA-256
`e8d38945a340228e04f51fec0f57af351772a4e3cabfbd9f079bdec14748bead`.
After: `probe-ts/captured-member-verified.json`, SHA-256
`f9a52f23b1bf9ade7c56a559c6a5a4b40d679823e66ae161738d6735c69b27c6`.
The separate offline graph audit `092-frontier/captured-read.audit.json` has
SHA-256 `7f1cfa8693474ee07e878ee3558d59025af54a79718df97e69d98a38be3d32b6`.

| Row | Candidates before → after | Gate outcomes before → after |
| --- | ---: | --- |
| Kobalte 0.9.2 | 33 → 33 | noop incomplete → incomplete; row refused |
| Kobalte alpha | 13 → 13 | both clamp gates completed → completed; 11 candidates withheld |
| i18n | 3 → 3 | census refusal → census refusal; all three gates unexecuted |

The exact source-gate refusal remains:

```text
solid-checker-rust: policy-2 case-set finalization failed: mandatory probe gate sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc did not complete
```

All six addressed gate IDs and their individual dispositions remain those in
`docs/2026-09-04-kobalte-js-graph-unlock.md`. Both clamp receipts retain their
accepted main digests and probe roots. i18n still refuses the same creates
census demand at `dist/index.js:3471..3483`, before launching any gate.
Total candidates stay **49**, structurally loadable JS candidates **9**,
completed gates **2**, and contradictions **0**. **0/43 original candidates
become newly probeable**; no additional real creates closure is accepted.
`exportsProven` remains **0** for all rows. No real contradiction was found;
the existing contradiction-veto regression tests continue passing.

| Check | Result |
| --- | --- |
| Pinned debug builds; probe harness | passed; 97 harness tests |
| Focused generated-read and forged-claim native test | passed |
| Backend library / IR library | 368 / 234 passed |
| Contracts / diagnostics / dialects process suites, armed | 7 / 15 / 37 passed |
| Contract corpus, non-updating final comparison | 89 passed |
| Coverage | 94 projects, 546 findings; unchanged |
| Ownership | 289 cases, 465 ledger rows, zero pending |
| Scripts Vitest suite, including phase19 | 153 passed; stable main count 180 |
| CLI tests and TypeScript type tests | 173 passed; types passed |
| Rustfmt then fmt check; workspace Clippy `-D warnings` | passed; separate pinned rebuild afterward |
| Schema parse, dialect manifests, diff whitespace | passed |

Cargo processes ran serially through the Makefile. No `make verify`, full
ecosystem benchmark, benchmark/phase20/phase21 repin, commit or push occurred.
All pre-existing worktree changes were preserved. The remaining next proof
issue is createReaction's callback flow; full captured-read execution modelling,
the 40 source-only candidates and the independent accessor/iteration census
remain open.
