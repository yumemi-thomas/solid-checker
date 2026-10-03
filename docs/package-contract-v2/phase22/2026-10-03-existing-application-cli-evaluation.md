# Existing-application evaluation of the reusable feedback CLI

Date: 2026-10-03

## Outcome

Three unchanged retained applications were evaluated with the current CLI and
their installed published typings. This trial exposes concrete integration,
precision and cost problems. It does **not** demonstrate a newly detected
application defect or establish an automatic-warning accuracy rate.

The two completed native analyses emit 19 findings classified as violations.
Review identifies 15 incorrect claims, two preferences and two loading-context
warnings that remain unvalidated. Both the main compiler build and the local
RC.13 candidate produce the same finding and proof-obligation arrays. The
larger application times out at the two-minute evaluation limit under both
builds; a timeout is not a clean result.

An isolated collector copy that restores the client website's existing alias
and static directory completes a real browser scenario. Its four assertions
pass, with no runtime errors or automatic guidance. Six retained read records
have no configured authored source frame. This is incomplete observation, not
proof of safe package use or evidence of six missed defects.

## Selection and preserved inputs

This is a convenience selection from already retained checkouts, chosen for
different application/tooling shapes. No package was installed, no typing stub
was substituted, and no application source was repaired or seeded with a bug.

| Application | Retained commit | Application inputs pinned | Installed TypeScript | Solid/web |
| --- | --- | ---: | --- | --- |
| `helge-dev` | `81f6cf1657563e775744119c785cffc265d6dea2` | 63 | 6.0.3 | 2.0.0-rc.9 |
| `oscartbeaumont-website` | `60823453a7e5e3aaa52179746d26abb46c7ff373` | 64 | 7.0.2 | 2.0.0-rc.9 |
| `finds-team/frontend` | `3c223e9282b5d6a61cfe5cf96be3709659270aec` | 200 | 5.9.3 | 2.0.0-rc.9 |

All three installed `tsc --noEmit --incremental false` checks pass. The feedback
adapter's separate TypeScript 5.9.3 check also passes for the two completed
configurations. Application source/configuration pins and original Git status
remain unchanged. The existing-test runners also verify their selected package
closures. The run records checker, producer, producer stamp, browser and adapter
digests; exact runtime inputs of the successful collector remain separately
bound. These user-writable records are evidence, not certification authority.

The main binary was confirmed current through `make build-checker-debug`, with
certification pins. The Type Facts source-manifest stamp matched and required
no rebuild. The RC.13 comparison uses the retained local candidate binary;
production compiler pins are unchanged.

## Current CLI and execution cost

Single-run wall times include the adapter's typing check and native process.
macOS `/usr/bin/time -l` supplies the recorded maximum resident-set measurement
for the measured command. These figures are not aggregate browser-process
memory, sustained-session bounds or performance guarantees.

| Application | Installed typing check | Stock feedback | Local RC.13 feedback | Completed native output |
| --- | ---: | ---: | ---: | --- |
| `helge-dev` | 0.43 s | 13.69 s; 425 MB max RSS | 13.55 s | 2 violations, 4 uncertifiable results |
| `oscartbeaumont-website` | 0.55 s | 9.03 s; 546 MB max RSS | 9.15 s | 17 violations, 11 uncertifiable results |
| `finds-team/frontend` | 1.64 s | Timed out at 120 s | Timed out at 120 s | No native output; classification unavailable |

The stock client live command exits 2 after 26.16 s, including its 12 s page
readiness timeout. Its original app works with its own Vite configuration.
The collector currently constructs `configFile: false`, `publicDir: false`
and supplies no application aliases. The other two applications have no
`index.html` entry and require server/generated-route configurations. Their
live runs are explicitly outside this collector's supported profile and were
not attempted.

The timeout's exact originating transform is not available: the stock CLI
discards the partial browser/transform report when a scenario throws. A
separate transform of the installed precompiled icon browser artifact refuses
its `solid-js/web` import without the existing alias and succeeds with it.
That establishes an alias compatibility boundary, not proof that this exact
artifact caused the stock readiness timeout.

`SOLID_CHECKER_TIMINGS=1` was enabled for the candidate comparison. The adapter
does not expose the native child's stderr, so there is no phase attribution
for the larger application's timeout. This should be resolved before guessing
which analysis phase to optimize.

## Review of every completed native finding

Native classifications are preserved in the raw reports. The independent audit
records a separate reviewed disposition for each exact finding location.

| Finding population | Count | Reviewed disposition and evidence |
| --- | ---: | --- |
| SC2003 nested invoicer writes | 14 | Incorrect dropped-write claim. `props.state` is the application's own mutable proxy, whose set trap delegates to `setStore`. Executing the unchanged helper's SSR branch retains both a nested client-name assignment and an increment through `props.state`. The readonly props container does not establish that these nested writes are dropped. |
| SC1001 `opened()` in the `Hamburger` callback | 1 | Incorrect rendering-time claim. The read is inside the callback passed to `Hamburger`, which forwards it to a DOM click handler. The original mobile tests exercise opening the menu and navigation closing it. This component-prop callback path needs an exact invocation-context proof. |
| SC8015 `prefer-show` | 2 | Preferences; no application defect established. Keep them separate when evaluating defect-finding value. |
| SC5003 loading warnings | 2 | Unvalidated. The client site's read is inside a lazy memo consumed under its existing `Loading`; the other site's route depends on an unresolved router/generated-route execution context. Neither warning is counted as a confirmed defect. |

The mutable-helper execution is a deliberately narrow behavioral check. It
does not execute the whole invoicer or establish its browser update/lifetime
correctness. The deferred-handler review does not establish every other
component callback's timing. No production rule is changed in this slice.

The 15 native uncertifiable results across the two completed configurations
include unknown package contracts, compiler-context gaps and a frozen-handler
proof obligation. Their presence remains visible. The feedback mode still
carries certification-oriented remedies in those native messages; this trial
does not accept contracts or turn their open claims into negative facts.

## Existing checks and configuration-adapted live run

Fresh checks against original tests:

- `finds-team`: 24 assertions pass across eight UI/Relay test files, with no
  skips. The original configuration regenerates only the copied route tree.
  The independent existing-test audit passes. Server-output UI assertions do
  not establish browser virtualizer or Kobalte lifecycle behavior.
- `helge-dev`: four original browser tests pass, covering the home headings,
  contact modal, mobile-menu opening and closing on navigation. The isolated
  copy retains the application's original Vite configuration; only dev-server,
  browser and report settings are adapted. Application and test source remain
  byte-identical. The test run takes 2.89 s, including runner startup.

The successful feedback run uses an isolated copy of the reusable collector.
Its only collector changes are `publicDir: "static"` and the application's
existing `solid-js/web` → `@solidjs/web` alias. Source models, runtime hooks,
selectors and native binaries are unchanged. This adapter remains an ignored
experiment artifact; it is not a production integration fix.

The scenario exercises home → projects → about → home → contact modal using
expectations already present in the application's tests. All four measured
heading/contact assertions pass; there are no page errors, blocked requests,
native runtime diagnostics or automatic notes. The run takes 13.77 s and
reports 813 MB maximum RSS for the measured command.

The hook records 804 read entries, 216 observer queries, six retained records
and zero dropped records. All six retained records are unmapped, so none is
admitted as an authored observation. Source instrumentation is present, but
the current report does not establish that relevant automatic-feedback
candidate paths were executed. This cannot be scored as zero-noise precision.
The initial unexecuted scenario also had a projects selector and contact
navigation mistake; both were corrected before the successful adapted run.
No assertions from the failed run are counted.

## Prioritized follow-ups

1. **Correct native overclaims before broadening adoption.** Add focused
   positive/negative regressions for nested mutable values passed as props and
   exact local component event forwarding. Required facts must establish the
   target proxy and invocation phase; otherwise leave the finding uncertifiable.
   Investigate lazy-memo/loading-boundary propagation separately.
2. **Integrate real application configuration and preserve failures.** Port a
   reviewed alias/public-directory profile, then define server/generated-route
   adapters. Return partial execution diagnostics on readiness failure instead
   of only a timeout string. Retain native stderr/timing output.
3. **Explain runtime coverage.** Preserve mapping-failure provenance and report
   entered/completed candidate scopes. Distinguish package-internal records
   without an authored frame from lost application attribution.
   *Done:* see "Explain what the run covered" in
   [development feedback](../../development-feedback.md). The six helge
   records are complete `@solidjs/router` package-only stacks, and the
   scenario entered none of the three derived-origin candidates.
4. **Measure the larger application by phase.** Attribute the timeout before
   optimizing. The completed small-project runs already argue for background
   analysis and reuse, rather than a whole cold process on each edit.
5. **Repeat on additional real interactions and defects.** This trial produced
   no confirmed application defect. Broader authored benchmark totals cannot
   replace independent real-app failures, reviewed warnings and side-effect
   checks.

## Reproduction and evidence

Reusable evaluation tools live under
`benchmarks/reviewed-package-models/development/`:

- `evaluate-existing-applications.mjs`: stock CLI, installed typing checks and
  selected existing test baselines; `native-only` compares another binary
  without repeating unchanged standalone typing/tests.
- `run-existing-client-tests.mjs`: the four unchanged original client tests.
- `review-existing-app-boundaries.mjs`: alias contrast and the unchanged mutable
  helper execution.
- `audit-existing-app-evaluation.mjs`: independent raw-result/source/profile
  checks, without importing the detector.

From the repository root, use fresh output paths:

```sh
make build-checker-debug
export SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust"
export SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"
node benchmarks/reviewed-package-models/development/evaluate-existing-applications.mjs \
  rust/target/existing-app-feedback-evaluation-N /absolute/path/to/chromium
```

The archived runners in both result directories reproduce the exact measured
versions. The final reusable runner includes the corrected scenario. Raw
outputs and generated copies remain ignored under:

- `rust/target/existing-app-feedback-evaluation-20261003-v1/`: stock results,
  installed typing logs, baseline tests, boundary review, collector profile,
  per-location finding review and independent audit.
- `rust/target/existing-app-feedback-evaluation-20261003-rc13-v1/`: candidate
  comparison, exact implementation pins and archived evaluation runner.

## Handoff verification

The independent evaluation audit passes: three published typing checks, 24
existing unit assertions, four original browser tests, four adapted live
assertions, exact source/model/profile bindings and the reviewed 19-finding
population. The independent existing-test audit also passes. Syntax checks
pass for all four new evaluation tools.

`make verify-fast` passes the current producer-stamp check, Rust formatting and
pinned workspace/all-target Clippy. `git diff --check`, schema JSON validation,
dialect manifest validation and new-file whitespace checks pass.

No production source, compiler pin, finding snapshot, accepted contract, schema
or dialect manifest was changed. Generated reports, source copies, browser
output and the isolated collector profile remain in ignored `rust/target`.
Full production verification, fixture coverage, ownership, contract and release
gates are intentionally deferred for this evaluation/tooling slice; their
historical results are not counted as fresh checks. The existing CLI suite is
not repeated because this slice changes no adapter or selector behavior.
