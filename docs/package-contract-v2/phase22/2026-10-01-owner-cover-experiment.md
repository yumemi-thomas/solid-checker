# Package-directed experiment: implementing a reusable owner proof

The experiment adds a bounded proof of the existing guaranteed owner-registration
claim, then tests ten offline certification transactions across eight published
packages. **18 consumer files pass published TypeScript typings; 36 consumer
analyses** compare authored receipts with the compiled-tier baseline. Runtime
and package versions remain pinned to the previous experiment's rc.9 installs.
The proof implementation is isolated on `codex/owner-cover-experiment`, in
`.claude/worktrees/codex-owner-cover-experiment`. The main verifier keeps its
previous behavior. Merging the prototype and promoting its claims are separate
steps requiring the full checkpoint and tier checks.

## Positive result

`@solid-primitives/event-listener@3.0.0-next.5:createEventListener` on browser
registers `createEffect` in one branch and `createRenderEffect` in the other.
The new proof covers their union on every normal completion. The exact source,
symbols and body frame are authenticated; no API-name heuristic establishes
the claim and neither individual call is marked unconditional.

The authored `min: 1` operation survives independent verification. SC4001 at
module scope changes from **uncertifiable to violation**, while the root twin
has no owner finding. Both still have SC9005 from other open domains. Node's
early server return and the host-free unresolved server branch both withhold
the stronger claim. A rejected stronger proposal does not recover the weaker
one automatically: host-free loses its possible owner finding but remains
explicitly uncertifiable through SC9005. This proposal must not replace the
existing weaker tier entry.

Browser RAF cleanup and memo reaction cleanup are replication controls: they
still prove their guaranteed registrations. RAF's recovery was already measured
by the initial pilot; memo was already in the compiled tier. Across the whole
pilot, two additional proven ownership findings have now been demonstrated:
RAF and listener. They remain experimental receipts outside the accepted tier.

## Breadth test

The previous browser checkpoint retains ten possible ownership operations across
five other packages. The second matrix strengthens each to `min: 1` in an
untrusted proposal and asks the same verifier to prove it. It also preserves
the six already-guaranteed operations in these documents as positive controls.

| Published package | Stronger possible claims attempted | Accepted |
| --- | ---: | ---: |
| lifecycle 1.0.0-next.2 | 1 | 0 |
| permission 2.0.0-next.2 | 2 | 0 |
| sensors 1.0.0-next.3 | 2 | 0 |
| timer 1.4.5-next.1 | 4 | 0 |
| workers 2.0.1-next.1 | 1 | 0 |

All six existing guaranteed operations survive. Four misuse/root pairs are
checked against real typings; sensors and workers have certification assertions
only, so this experiment makes no consumer-diagnostic claim about their APIs.
Every checked correct-use consumer remains SC9005 uncertifiable. No additional
complete package or clean export is demonstrated by these matrices.

These refusals must not be read as ten missing branch joins. Some functions
actually skip registration on a valid path. Timer selects cleanup registration
for a numeric delay and effect registration for a callable delay: their common
ambient-owner obligation does not establish that either individual operation
happens on every call. Other targets need helper, loop, callback or source-identity
proofs beyond this bounded grammar. Increasing the number in a proposal cannot
establish an operation the source does not guarantee.

## Assessment

The positive result is evidence that an existing claim can gain useful behavior
from a reusable proof without relaxing soundness. This is a reason to keep the
verifier architecture. It does **not** support optimism that package-directed
authoring alone will reach the all-package checkpoint: this implementation
recovers one additional finding, and the breadth test recovers none elsewhere.

The current manual-proposal strategy is inadequate as the primary scaling plan.
Further work should target compositional effects and dependency/callback/value
provenance that close several domains together, with a whole-package outcome as
the success criterion. The [domain inventory](2026-10-01-package-directed-completion.md)
shows 454 browser exports blocked across all four domains. That is stronger
evidence of the bottleneck than a count of successfully authenticated partial
receipts. Returned callable-member support remains an owner decision before
implementation; this experiment adds no supported value shape.

## Evidence and checks

The final prototype replay is retained under `rust/target/package-directed-owner-cover-final/`
and the five-package matrix under `rust/target/package-directed-owner-bounds/`.
Append `cover` or `bounds` to the runner's fresh output directory. `check-cover.mjs`
verifies either saved observation without repeating certification. The cover
has focused positive and negative syntax tests and source/symbol/frame binding
tests, including missing else, early return, throwing-only, catch, loops,
optional and nested calls, wrong roles, wrong symbols and wrong frame spans.

No accepted-tier artifact, Type Facts protocol, schema, dialect, finding snapshot
or misuse ledger is changed. Global checkpoint and app measurements have not
been rerun; the last checkpoint remains the reference, not a post-change census.
Tier promotion requires the normal census, host runs, checkpoint, accepted-bundle
generation and embedded authentication checks together.

Prototype validation passed: the armed Rust workspace tests (1,568 tests, no
failures or ignored tests), the 142-project coverage comparison (737 findings),
the 120-fixture contract corpus, and the ownership gate (41 cases, 465 ledger
rows, no pending rows). Focused syntax-cover and source-binding tests passed.
Formatting, workspace Clippy with warnings denied, schema/manifest validation,
and diff whitespace checks passed. Both saved experiment matrices pass their
observation assertions. The full release verification, scripts suite, checkpoint,
tier regeneration and app sweep are deferred because the implementation stays
on an experimental branch; this is not a release or tier-promotion result.
