# Existing package hydration failure and source feedback

## Outcome

There is now a reproduced package integration failure beyond TypeScript in an
existing application test. The retained `finds-team/frontend` Kobalte candidate
passes its configured TypeScript project, then fails the original production
hydration test. The native control passes the same test and CSP. No application,
test, package or runtime source is repaired or instrumented for these runs.

This supports cautious optimism about useful debugging feedback without a
handwritten behavioral contract for each package. It does not establish warning
precision across packages. The candidate is already documented as incompatible
and is unused in the active application. This is a reproduced existing candidate
failure, not a newly discovered defect in the active application.

## Existing test and observed behavior

`real-package-hydration-test-v1.mjs` authenticates and copies the application's
200 inputs and reuses retained dependencies. It runs the original
`e2e/kobalte-hydration.spec.ts` hydration test under a narrow generated
configuration: one original fixture server, one original test, one worker and
the installed browser. The original builds, aliases, CSP and test source remain
unchanged. The candidate enables the application's existing
`KOBALTE_COMPATIBILITY=1` switch; the control disables it.

| Observation | Candidate | Native control |
| --- | ---: | ---: |
| Configured TypeScript project exit code | 0 | 0 |
| Original selected browser test | Failed | Passed |
| Skipped tests / retries | 0 / 0 | 0 / 0 |
| Hydration completion marker | Absent | `true` |
| Browser hydration errors | 2 | 0 |
| `[REACTIVITY_HALTED]` console messages | 2 | 0 |
| Blocked inline-style CSP messages | 4 | 0 |
| Exact mapped client diagnostic frames | 22 | 0 |

The failure is in the original test's hydration prerequisite at line 15 and
its clean-console assertion at line 19. The candidate never reaches the test
body at line 22. Retained server DOM identity in the separate observer does
not establish successful hydration when the completion marker is absent.

The actual project is strict and includes the candidate TSX source under
`src`. Both typing runs use real installed package declarations; no fixture
stubs replace them. The project retains `skipLibCheck`, as its original
configuration specifies. These are no-error observations for that configured
project, not a new checker rule or proof that every package declaration is valid.

Retained versions include Kobalte `2.0.0-alpha.2`, Solid/signals/web
`2.0.0-rc.9`, Vite `8.3.0`, vite-plugin-solid `3.0.0-next.27`, TypeScript
`5.9.3` and Playwright test `1.63.0`. Kobalte's declared peer combination is
RC.3; the application's RC.9 override is part of the tested installed
combination. No version is silently substituted.

## Exact source links, with responsibility left open

`real-package-hydration-observe-v1.mjs` observes the original fixture server
and production client in Chromium. It independently builds a hidden source
map and accepts source links only when its generated client is byte-identical
to the original server's served bundle. Every mapped source's embedded content
must also match its file on disk. Candidate and control satisfy both checks,
with 75 and 33 exact source files respectively.

The initial attribution trial refuses the map because Rollup's region comments
reflect different working directories. Matching the original server's working
directory makes all client bytes identical. The failed trial remains preserved;
comments are not stripped and the identity requirement is not weakened.

All 22 captured client diagnostic frames map to Solid runtime artifacts:
`@solidjs/web`, `solid-js` and `@solidjs/signals`. Kobalte appears in the
candidate bundle, but no captured frame establishes it as the cause. File
ownership is package artifact ownership, not exact export dispatch or causal
responsibility. The four CSP messages refer to document locations and remain
outside the client source map. No source repair is proposed.

`real-package-hydration-audit-v1.mjs` independently checks the raw original
test outcomes, assertion locations, typing command/results, package closure
digests, source bytes, served/map byte identity, source-map coordinates and
browser diagnostics. It emits one informational note attached to the existing
failed test and none for the native control. The note states the observed
integration failure and links exact runtime locations; causal package, static
dispatch and repair remain open. This is a research feedback artifact, not a
production finding or package certification.

## What this changes about confidence

The earlier capture experiment made ten authored failing value assertions pass
and stayed quiet on 22 controls under an adapted policy. This trial adds an
existing package integration failure with unchanged source and a passing native
control. Together they support a practical test-assisted workflow: collect the
failure, bind it to exact installed bytes, show source locations, and evaluate
any proposed change in an isolated replay against existing assertions.

The shared collection and mapping mechanisms can work across package artifacts
without manually certifying each one. Coverage still depends on executed
interactions and available source maps or analyzable source. A quiet run proves
nothing about unexecuted behavior. Tests provide expectations for observed
interactions; they do not establish all package semantics or make repairs safe.

General automatic warning precision, a positive defect in active application
code, indirect package dispatch, broader callback/result flow, startup cost and
total memory bounds remain open. The two earlier noisy raw async controls remain
unresolved. No result here establishes feedback for all packages or all rules.

## Verification and artifacts

Focused checks are the two original selected browser tests, two actual typing
CLI runs, two browser observations and the independent paired evidence audit.
The expected candidate test failure is the result being reproduced; the
independent audit passes. Prior prototype modules remain unchanged and their
823 tests are not rerun or counted as new application evidence. Syntax checks
and comparison with the previous handoff seal cover the research modules.

Ignored outputs include:

- `rust/target/real-package-hydration-{candidate,native}-v1/` for isolated
  application copies, typing results, original test JSON and logs;
- `rust/target/real-package-hydration-observed-candidate-v2/` and
  `rust/target/real-package-hydration-observed-native-v1/` for browser evidence,
  served client bytes and independently bound source maps;
- `rust/target/real-package-hydration-observed-candidate-v1/` for the refused
  initial mapping trial;
- `rust/target/real-package-hydration-audit-v1.json` for paired source and
  outcome validation and the informational feedback artifact.

Fast handoff verification checks the producer stamp, Rust formatting, pinned
workspace Clippy, schema/manifest validation and whitespace. Full production
verification, coverage, ownership, contract and release gates remain deferred
for this research/documentation slice. No production code, snapshot, contract,
schema or manifest changes. No installation, network acquisition or external
message is used.
