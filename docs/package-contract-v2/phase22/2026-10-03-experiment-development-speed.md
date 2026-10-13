# Experiment development speed

## Outcome

A new focused development command completes in **21.27 seconds**, including
40 tests, eight observed cases and an independent audit. It reuses the
authenticated saved plain baseline. The preceding full 58-case cycle took about
189 seconds: 108 seconds observed, 60 seconds plain and approximately 21 seconds
for the audit. The focused cycle is roughly nine times shorter because it runs
fewer cases and avoids an unchanged plain run. This is not a speedup of the
analyzer itself or a replacement for full population validation.

The focused run reproduces four detected targets, two quiet named-object
controls and two remaining noisy identity controls. It is explicitly marked
`developmentOnly: true`. Selecting old cases adds no fresh-case evidence.

## Where time goes

The eight final browser runs from the preceding experiment have recorded
start/finish timestamps totaling 365.32 seconds, about six minutes. Earlier
failed/replaced trials add further execution time. Focused semantic tests usually
take less than a second individually; the saved universal fast check took about
three seconds. Rust build time was not the principal cost in that slice.

The new command records its phases directly:

| Focused phase | Measured time |
| --- | ---: |
| 40 focused tests | 1.35 s |
| Eight-case observed browser process | 16.54 s |
| Independent audit | 3.38 s |
| Complete command | 21.27 s |

Within the browser process:

| Recorded activity | Measured time |
| --- | ---: |
| Initial navigation, including served transforms and readiness waits | 10.04 s |
| Feedback projection | 2.95 s |
| Explicit update waits | 0.65 s |
| Post-navigation type-fact lookup | 0.28 s |
| Browser launch | 0.27 s |
| Package and profile authentication, both boundaries | 0.23 s |
| Tool imports | 0.22 s |
| Source mapping | 0.09 s |
| Report writes | 0.07 s |

Navigation includes application compilation and initialization. These timings
do not attribute all ten seconds to waiting or all projection time to one
semantic model. The runner's `networkidle` readiness adds a quiet-network wait
on each navigation. Replacing it requires an application completion signal and
paired behavior validation; removing it blindly would change when updates occur.

The previous 58-case observed run also records 11.05 seconds in additional
package source-program builds and 4.76 seconds in their input validation.
Published typing construction is not included in those two counters.

We do not have reliable accounting that separates model reasoning latency from
manual orchestration, editing, tool round trips and documentation. Saved
artifact timestamps cannot supply that split. The experiment's 31 browser-runner
versions, 28 projectors, 27 plugins and 18 auditors do show substantial process
complexity. Managing those families and manually assembling each report adds
work beyond executing the tests.

## Changes made

`feedback-revision-browser-v32.mjs` preserves V31's semantics, browser waits and
per-case isolation. It adds phase measurements and optional exact case-ID
selection against the original case module. The selection file is authenticated
at both run boundaries. Package and source hash checks remain intact.

Independent audit V19 retains V18's semantic reconstruction. It checks the
selected observed population exactly and accepts either a matching focused
plain population or the complete original plain population. The old full
baseline still authenticates its case source, package closure, frozen profile
and behavior; the audit compares every selected case against it. No report is
edited to pretend that old execution used a new profile.

`development-case-selection-v1.mjs` refuses unknown, repeated, empty and malformed
selections; extra, missing and repeated observed/baseline rows; duplicate source
case IDs; and incorrect development metadata. Eight new tests cover those paths.
The full focused command runs 40 tests with no failures or skips.

`experiment-dev-loop-v1.mjs` performs the focused tests, observed run and audit
as one command. It saves logs, phase times, browser times, exact selected IDs,
baseline hashes and the audit identity. Each invocation needs a fresh output
directory. Failure stops the sequence and records its phase. A missing baseline
in a custom configuration causes an actual focused plain run.

## Use for the next change

Run from the repository root:

```sh
node benchmarks/reviewed-package-models/experiment-dev-loop-v1.mjs --plan
node benchmarks/reviewed-package-models/experiment-dev-loop-v1.mjs \
  rust/target/development-focus-N \
  /absolute/path/to/installed/chromium
```

The default selection covers the four former misses, two named-data controls
and two intent-boundary controls. An optional third argument names a JSON
configuration with `cases`, `freeze`, `baseline`, `caseIds` and `tests`.
Use a newly frozen current profile after changing detector code. The runner
never silently rewrites a seal whose source no longer matches.

Use the focused cycle while changing one semantic branch. Run the full original
population, a genuinely fresh challenge, replay/assertion checks and required
handoff gates once the branch is ready. Passing the eight-case development
selection establishes only those eight cases.

Further improvements should target:

1. Application readiness/completion signals to shorten browser waits while
   preserving the measured interaction.
2. Offline re-projection of authenticated observations for selector-only edits;
   browser execution remains necessary when instrumentation or source changes.
3. Reuse of invariant typing/source work with complete content-based input keys.
   Package authentication is already inexpensive and should remain strict.
4. One active development profile, with immutable evidence snapshots at review
   milestones, so each small edit does not require another chain of copied
   runner/plugin/projector files. Earlier sealed evidence remains intact.
5. One hypothesis and an explicit success criterion per slice. The established
   intent boundary should be treated as settled unless new evidence changes it.

These are follow-up opportunities, not measured improvements from this patch.

## Evidence and handoff

Saved result: `rust/target/development-speed-focus-v1/results.json`.
The independent audit and phase logs sit beside it. Browser timings are also in
`observed/results.json`. The new development profile is
`rust/target/development-speed-detector-freeze-v1.json`.

All 622 previous code/input pins and 61 previous artifact pins remain unchanged.
The new selection tests and the complete focused run pass. Syntax, whitespace,
universal fast checks, schema and dialect manifest validation pass. Full
production coverage, ownership, certification and release gates remain deferred
for research-only runner changes. No production rule, fixture snapshot,
published contract or compiler pin changes. Generated case copies and reports
remain under ignored `rust/target`.
