# Composed callable experiment: a complete node package case

Date: 2026-10-01. Implementation is isolated on
`codex/composition-callables`, in `.claude/worktrees/codex-composition-callables`.
No production verifier or accepted tier has changed.
Implementation commit: `2726e7f94`, based on delivery branch commit `e6eb94400`.

The existing described-callable shape now composes an exact local helper and
an exact dependency's authenticated claims. The published
`@solid-primitives/cursor@1.0.0-next.2` trial, pinned to Solid rc.9, closes all
six node exports. Its two whole-surface consumers and callback control receive
`status: certified`, accepted package evidence and no findings. This is the
first additional whole-package host case demonstrated by these experiments.

| Host | Compiled tier | Authored, independently verified graph | Complete |
| --- | ---: | ---: | --- |
| node | 3/6 clean | 6/6 clean | yes |
| browser | 0/6 clean | 0/6 clean | no |

Six strict checks against TypeScript 5.9.3 and the published package declarations
pass. Twelve consumer analyses compare baseline and authored receipts. Browser
findings agree exactly: three owner violations in the unowned whole-surface
sample and one in the callback sample, with no owner violation in the rooted
twin. Browser still reports incomplete contracts and unresolved returned-ref
dispatch. These four owner violations already existed; this slice preserves
them rather than claiming four new findings.

## What composes

`cursorRef` returns a literal calling the local `createElementCursor`. The
existing transitive census already proves that helper's node branch does
nothing and completes without a value. The prototype acquires and follows that
exact stable helper instead of refusing it, and states a plain undefined result
using the existing shape. Recursion, premised helpers, captured callback effects
and owned reads through helpers remain refused.

The two `make*Cursor` functions return utils' imported `noop`. The verifier
binds the exact completion and import reference against authenticated source,
then requires an exact graph edge and the dependency's own verified export.
Four obligations require callbacks, reads, creates and returns to authenticate
and remain closed in the child's receipt. Unknown or withheld behavior cannot
become a negative claim in the parent.

The published `noop` is `() => void 0`. Its explicit undefined result is a
plain return. An earlier deliberately incorrect empty-returns proposal was
withheld, leaving the two parent returns incomplete. The contract distinguishes
an explicit value-carrying completion from a valueless completion even when
JavaScript evaluates both to undefined.

An exact-runtime node observation calls all six exports with a throwing DOM
proxy and throwing callbacks. No DOM property or callback is reached, and the
ref callback returns undefined. This falsifier supports the test design; it
supplies no proof authority.

## Controls and validation

Final positive evidence lives in
`rust/target/package-composition-cursor-final/results.json`. The directory keeps
proposals, graph inputs, native output, receipts, trust configuration, raw
consumer output and binary identities. Saved assertions additionally require
the node consumer's certified status and accepted package evidence.

The `open-dependency` mode removes the child's callback closure. The
`withheld-dependency` mode proposes the incorrect empty return and correspondingly
empty returned-function result in the parent. Both must leave the two dependent
exports incomplete while retaining the local-helper gain. The second control
tests withdrawal after child certification, not merely missing input before
proof. Each mode includes three published-type checks, six consumer analyses
and the same runtime falsifier.

Together the final positive trial and both refusal controls pass twelve strict
published-type checks and twenty-four consumer analyses. The positive trial
adds one complete host case; the controls add none. No all-host package is
newly complete.

The isolated worktree's final `make verify` passed in 823.29 seconds, exit 0,
with `TOTAL` present and no `FAILED during step` marker. It passed 1,568 Rust
tests, Go race checks, formatting and Clippy; compared 142 fixture projects and
737 findings without snapshot changes; checked 41 ownership cases and 465 ledger
rows, 120 contract fixtures, 102 TypeScript/checker oracle cases, and six
obligations with ten discharges. Performance, CLI, scripts, schema and manifest
checks passed. Bundle conformance has zero active receipt-issued bundle cases;
the experiment's own graph admission and consumer tests supply the evidence for
these authored receipts. No accepted artifacts or snapshots changed.

The final release build and saved positive/refusal assertions passed. The three
new backend tests exercise exact import identity, all four dependency domains,
and helper refusal for missing identity, values, async completion and missing
flow. A new syntax-fact test binds statement and arrow completion ranges.
JavaScript syntax and whitespace checks passed. The timer baseline separately
passed eight published-type checks and sixteen consumer analyses.

Initial checks exposed and corrected an oversized syntax fact and an incorrect
arrow-return test assumption. Completion span rows now live separately,
preserving the existing compact ReturnFact layout. Earlier broad checks were
blocked by a denied Go cache write and missing retained TypeScript lock metadata.
The successful run used a cloned writable Go cache and the audited installation
with its original lock; no new packages were installed to investigate a semantic
failure. The complete gate log is retained at
`/private/tmp/solid-checker-composed-verify-complete.log`.

## Interpretation

This provides a concrete reason for cautious optimism: reusable proofs compose
into a complete package case without introducing a public claim form or
loosening uncertainty. It does not demonstrate complete browser behavior or
general callback composition. Node is the simpler case because these APIs
intentionally disable their effects there.

The proposals are authored. Automatic generation and the committed tier do not
yet contain these additional descriptions, so the global all-host checkpoint
and app-import metric have not improved. Automatic proposal generation is the
next scaling test. Before a production merge, regenerate the complete retained
census/environment/checkpoint/bundle chain and authenticate all embedded bundles.

Timer's retained factory-callback result is a separate expressiveness gap. Its
new form remains awaiting the owner's decision; this experiment uses existing
forms only. Browser DOM member behavior, objects/arrays, and remaining callback
attribution still require their own proofs.
