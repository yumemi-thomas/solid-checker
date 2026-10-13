# Automatic ownership breadth and awaited member calls

This continuation establishes two concrete improvements:

- Automatic lifetime feedback now detects **14 of 14 authored targets** across
  six packages, with no new lifetime warning on thirteen controls.
- A declaration-driven sample automatically selects **17 ownership cases across
  twelve packages**. All seventeen are detected at runtime and by experimental
  static source models; all seventeen owned controls are clean.

The two package groups are disjoint. These results support shared diagnostics,
resource monitoring and positive source premises as a scalable foundation.
They do not establish coverage of most packages or every incorrect use.
Warnings from source models and lifetime policy still have no certification
authority. No production analyzer rule or accepted contract changed.

## Closing the awaited member gap

The native continuation transform now captures a member callee before evaluating
its arguments. Its runtime bridge evaluates the receiver and property once, then
invokes the original function with the original receiver. An awaited receiver or
computed key remains in its native async function. No Promise reaction is added.

The previous `Timer.makeTimer(callback, await delay, setTimeout)` miss now emits
the lifetime warning at the original caller. A fresh comparison executes the
27 lifetime consumers and cached app under original and native profiles:
**56 browser executions**, fourteen targets detected, thirteen controls without
a new lifetime warning, and preserved scheduling arrays. The app retains the
same existing `UNSTABLE_MEMO_OUTPUT` advisory in both profiles and completes its
existing flow. That advisory is not attributed to the new monitor.

Focused tests verify awaited/computed keys, property conversion order, getter
order, callback receivers, primitive receivers and thrown getter identity. Super
and private member calls with awaited operands, optional chains, awaited
constructor arguments, direct eval, async generators and for-await remain open.
The earlier generator rewrite remains rejected; it was not rerun.

## A broader sample without per-export test recipes

The extractor now follows the actually resolved Solid runtime through transitive
dependency layouts, rather than requiring signals and web at the consumer root.
The fresh inventory covers 97 retained package roots:

| Source measure | Result |
| --- | ---: |
| Roots with positive premises | 56 |
| Exports with a positive browser or node premise | 151 |
| Exports with a browser ownership premise | 91 |
| Packages with those browser ownership premises | 42 |
| Refused root entries | 3 |

Animation, controlled-props and virtual remain refused because their recorded
root entry is missing or escaped. An empty model is never substituted.

From the 91 ownership-premised exports, the selector resolves the actual
published export symbols and callable signatures. Seventeen exports in twelve
packages admit a zero-argument call. The other 74 are left unexercised; no
arguments are guessed or loosened through fixture stubs.

The selected packages are a11y, clipboard, cookies, date, devices, drag-drop,
focus, fullscreen, idle, mediastream, styles and upload. Each selected call has
an unowned and owned browser specimen. All **34 specimens** pass real published
typing, execute without environment failures, and produce the expected
`NO_OWNER_CLEANUP` or `NO_OWNER_EFFECT` warning only in the unowned version.
Returned accessors are not read, so this experiment tests construction ownership,
not later reactivity, readiness or browser permissions.

This is source-selected positive sampling. It is not a blinded recall test of
every export, and zero-argument defaults cover only a small part of each API.
The 91 source premises are assumptions with source references and input pins,
not 91 demonstrated detections or behavioral proofs.

## Static feedback and a composability defect

The first static comparison detects thirteen of seventeen targets. Four misses
come from clipboard, cookies, date and mediastream: the adapter refuses to model
an entire returned tuple when there is no destructured accessor binding, and
that refusal also discards the independent ownership premise.

The corrected adapter applies a zero-argument ownership premise independently.
It preserves the original call and published result type, marks only ownership
as applied, and retains an explicit gap for the opaque tuple result. It supplies
no reactive-read fact from this partial model. Withheld owner facts remain
unmodeled. Both original and analysis specimens pass published typing.

Owned static controls use a block inside the real core `createRoot`, ensuring
the ownership premise is actually applied there. A quiet control with an
unapplied model is insufficient. The final comparison detects **17 / 17**
unowned targets with **0 / 17** owned-control warnings; all 34 specimens exercise
their ownership premise. Baseline analysis has no corresponding proven finding.
Native uncertifiable results remain in the raw outputs; a clean control here
means no projected model warning, not certification of the package.
The additional warnings remain `source-extracted-assumption`, with
`certification: false`.

## Measuring the shared-project path

The retained installs are separate, so simply using the first installation for
the batch fails on a missing clipboard package. A temporary type-only project
links each exact retained package and authenticates its closure. This does not
execute the assembled graph or prove runtime deduplication between packages.

One fresh project contains all seventeen unowned calls and seventeen owned
controls:

| Measured step | Time |
| --- | ---: |
| Published typing checks, lowering and analysis-twin typing | 0.526 s |
| Baseline checker analysis | 0.851 s |
| Modeled checker analysis | 0.852 s |

The batch retains all seventeen warnings and no owned-control warning. Input
authentication before the preparation timer is outside these figures. These
are one-run measurements of a small synthetic project, not editor latency,
end-to-end extraction cost, a large-app benchmark or a claim that all projects
can combine these installation graphs unchanged.

## Evidence, limits and the next useful scope

This continuation executes **90 browser specimens** and **142 fresh native
analyses**: an initial 68-analysis static comparison, four focused analyses
after the ownership slice, a final 68-analysis comparison with stronger controls,
and two batch analyses. The intermediate run explicitly reuses unchanged
observations; those reuses are not counted as fresh.

Browser runners preserve input byte copies and compare before/after manifests:
sixteen local inputs for the lifetime comparison and seventeen for the ownership
sample. The ownership sample's frozen `lower.mjs` predates the static helper
change. The validator records that historical input difference, verifies its
preserved bytes, and checks current package pins and declaration bytes. Its module
initialization is unchanged; the browser harness imports its TypeScript export
and does not execute the changed modeling functions. Historical runtime evidence
is not relabeled as a fresh execution of the newer helper.

Artifacts:

- `rust/target/automatic-life-members/` and
  `rust/target/automatic-life-members-validated.json`.
- `rust/target/zero-argument-source-catalog.json`,
  `rust/target/zero-argument-selected-with-projects.json`,
  `rust/target/zero-argument-owner-browser/` and
  `rust/target/zero-argument-validated-final.json`.
- `rust/target/zero-argument-static-strong-controls/` and
  `rust/target/zero-argument-batch-linked/`.

The principal remaining breadth limit is the 74 ownership-premised exports
requiring arguments, plus source branches and exports with no positive premise.
Concrete consumer calls are needed to exercise argument-dependent behavior.
Static partial ownership currently admits the zero-argument tuple case; other
opaque return placements are not established. Runtime feedback requires execution.
Unexecuted paths, arbitrary callback delivery, dependency async bodies, custom
component event forwarding, multiple realms and runtime graph identity remain
open. Implicit resource retirement through garbage collection is not observed
by this registration monitor. Lifetime warnings still need declared project intent; stale-read defects
still need intent or an explicit behavior expectation.

The next useful investigation is to widen argument-bearing consumer specimens
and representative app paths, then measure missed defects and control noise.
Adding source premises alone does not establish that coverage.

Validation passed: **68 prototype tests**, both saved-study validations, the
strict original/analysis typing checks, the batch assertion, Rust formatting,
pinned workspace Clippy, diff whitespace, schema parsing and dialect manifest
validation. Full repository verification, certification, coverage and ownership
gates were deferred for this isolated experimental change. No public contracts,
receipts, schema or fixture snapshots changed.
