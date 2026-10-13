# Real application execution and feedback admission

## Result

The first retained application now runs with the prototype, preserving **39
tested UI states** against its plain version. Its two entered candidate scopes
produce no observations or hints. This is useful integration and narrow control
evidence. It does not establish positive defect detection or an accuracy rate
in real applications.

A current-source inventory also covers nine retained configurations. Eight
can be analysed: five type-check, three have existing published-typing errors
and are closed for feedback admission. The ninth lacks `@solidjs/web` and is
refused. None of these inputs is repaired or given substitute declarations.

The inventory shows a gap between broad package demand and this memo-focused
feedback path. Among the five clean configurations, there are 2,620 call
references outside recognizable test files and typings packages, plus 177
external JSX references. There are 78 candidate sites; **none directly matches
one of the external declared calls by source span**. Local helpers and indirect
package flows remain possible. These figures do not prove an external call is
misused, executed or supported by runtime source enrollment.

The result supports continuing the combined package-feedback design. It does
not support making this one late-memo-read path the whole product.

## Application and exercised behavior

`helge-dev` is retained at Git commit
`81f6cf1657563e775744119c785cffc265d6dea2`. Its worktree is clean. The trial leaves
its application, tests, typings and package bytes unchanged.

The adapted development configuration preserves the application's `static`
directory and existing `solid-js/web` icon alias. It disables component HMR
and dependency prebundling, uses the retained compiler/runtime, and adds an
observation entry before the application's existing entry. Cache and reports
live outside the app tree. This is a reviewed client development profile, not
a production deployment or general framework integration.

Actions come from the existing browser tests and source: home, projects,
about, desktop navigation, contact modal open/close, mobile menu navigation,
not-found handling and twelve repeated contact open/close cycles. Clipboard
copy, sending email and external links are not activated.

The first V1 run exercises 36 steps without the blog. It completes without
browser errors or hints, but does not establish candidate execution. V2 adds
blog/article actions with explicitly authored offline HTTP data. Two sample
articles exercise the existing blacklist; the article route receives sample
HTML. The same data and bytes are supplied to plain and instrumented runs.
They are not responses fetched from the real service. Other remote requests
are blocked; none occur in the final run.

V3 adds a random page-load identity to observation bookkeeping. Normal document
navigation resets the runtime counters; summing every UI snapshot would count
the same reads repeatedly, while looking only at the last snapshot would lose
earlier entered candidates. The independent audit checks matching document
transitions and sums the last counters of each load.

The final comparison covers:

- 39 matching UI snapshots across three document loads and two transitions;
- 2,304 native read entries and 162 tagged native identities across those loads;
- two entered, normally returned candidates, both on the offline blog route;
- zero candidate observations, hints, continuation gaps or native diagnostics;
- 22 transformed modules, three enrolled sites and 32 async helper definitions;
- 28 enrolled package modules and 2,061 authenticated analysis input records.

The other two enrolled sites are in the router's action source. Enrollment
does not imply entry. During the repeated contact steps, event/seen/gap counts
remain zero and retained metadata stays at six records. These short quiet
repeats do not test buffer growth under many positive observations.

## Real-source inventory

The independent reference audit reconstructs each published typing program
and checks all recorded source/declaration/metadata identities. It validates
4,973 call references and 261 JSX references across 819 configured source files
in the eight analysable configurations.

The first inventory included source-located tests and ambient APIs owned by
`bun-types` and `@types/node`. V2 reports recognizable test filenames separately
and calls the packages **declaration owners**. It does not guess the executing
package from a declaration's name. Runtime provider binding stays unresolved
and dispatch stays open. Other configured source still includes server code,
tooling and local helpers; this is a source-demand study, not a runtime census.

| Clean configuration | Source files | Call references outside named tests and typings packages | Other-source external JSX | Candidate sites | Enrolled runtime modules |
| --- | ---: | ---: | ---: | ---: | ---: |
| oscartbeaumont-website | 20 | 18 | 24 | 3 | 29 |
| sefer | 324 | 2,506 | 139 | 69 | 2 |
| finds-team | 132 | 70 | 3 | 2 | 45 |
| helge-dev | 18 | 3 | 11 | 1 | 28 |
| expenses-app | 54 | 23 | 0 | 3 | 0 |

The five configurations contain 548 source files, including 50 test-named files.
Their other-source call references total 2,782 before excluding typings owners.
No candidate span directly coincides with a recorded external call. This
checks a precise source relationship, not complete package coverage or whether
local candidates delegate to an external implementation.

All eight analysable configurations still reach an existing source-discovery
boundary. Runtime-module counts are partial enrollment; they do not measure
call coverage. The largest clean project has 69 candidates but only two enrolled
runtime modules. Most configurations have not been executed: authentication,
server integrations, automatic/generated build inputs and missing artifacts
need separate reviewed runtime profiles.

## Cost and measurements

The final application run's first tested screen takes about **4.05 seconds**
with observation and **0.90 seconds** plain. Total run times are 18.33 and
15.09 seconds, including UI waits and repeated close timers. These are single
cold development observations with multiple concurrent processes, not latency
guarantees or a performance certification.

The source session builds once. Its recorded source build takes about 1.04
seconds; validation totals about 0.95 seconds across 22 reuses. Native hook,
compiler, Vite and public typing costs also contribute. Collected Node heap is
about 104 MB observed versus 91 MB plain; process RSS is about 1.22 GB versus
0.77 GB. The memory figures are runner observations, not live browser memory
or a bounded long-session guarantee.

Clean inventory runs take roughly 1.51–10.83 seconds, including typing/source
construction and selection. This argues for cached/background analysis and
scoped runtime sessions; it does not establish acceptable per-keystroke cost.

Runtime V9 adds entry/normal-return counters and retained event, seen, metadata
and gap counts to V8. Normal return means the candidate call returned; it does
not claim a returned Promise fulfilled. Callback runtime V4, continuation
transform V6 and plugin V23 preserve the prior algorithm and use that runtime
consistently. Global evidence and metadata remain unbounded.

## Verification and versions

Eight focused measurement tests verify entry versus normal return, nesting,
throws, exact repeated-read counts, budget refusal, Promise identity/scheduling,
detached statistics and retained metadata counts. The full prototype passes
**682 tests with no skips**.

A 46-case async queue replay under the measurement profile retains **14/18
target hints, 24/26 quiet controls and two silent published-typing exclusions**.
Independent source/frame/plain-behavior audit V11 validates all 46 variants.
Object/Promise-adoption targets remain missed and two constant controls remain
noisy. Other earlier populations are not replayed in this slice.

The new real-app browser audit imports no detector. It validates unchanged
application/package inputs, matching visible/native-diagnostic behavior and
load transitions, current manifests, published and implementation typing
programs, exact site declaration spans, helper source spans, input/source pins
and measured counters. It does not validate complete discovery, complete
candidate selection or positive real-app defects. The reference audit also
imports no selector and validates reported references, not completeness.

Actual `tsc --noEmit --incremental false` is clean for all five clean
configurations against their installed typings. No declaration stubs, package
installs or remote acquisition are used. Syntax checks pass for 481 modules;
53 historical/current seals authenticate 491 distinct pinned paths.

Fast handoff checks pass: producer source stamp, Rust formatting, pinned
workspace Clippy, schema JSON, dialect manifests and `git diff --check`.
Full production verification, fixture coverage, ownership and contract corpus
gates are deferred for this research-only slice. No production Rust, schema,
contract, manifest, compiler lowering or finding snapshot changes. Generated
apps, traces, inventories, checks and seals stay in ignored `rust/target`.

## Next requirements

The next experiments should cover additional execution/ownership families,
component and effect integration, and genuine or independently reproduced
defects in applications. They must retain exact declarations, real published
typing gates, explicit unknown boundaries and differentiated feedback strength.
The remaining object/adopted async results also need weaker body-completion
evidence studied separately from Promise fulfillment and consumer-result proof.

Runtime coverage reporting should show whether candidate paths were entered.
Positive/quiet benchmark totals alone can hide an inactive detector. Practical
rollout still needs representative real-app precision, useful explanations,
source-discovery and storage breadth, cold-load/interaction costs and bounded
long-session retention. Feedback stays informational with no certification
authority; this is progress toward useful package feedback, not completion of
universal package/rule coverage.

## Evidence

Repository-relative paths:

- `rust/target/real-app-read-browser-{reads,plain}-v{1,2,3}/results.json`.
- `rust/target/real-app-read-browser-audit-v1.json`.
- `rust/target/real-app-read-admission-v{1,2}.json` and
  `rust/target/real-app-read-admission-audit-v1.json`.
- `rust/target/real-app-read-prior-async-browser-reads-v1/results.json` and
  `rust/target/real-app-read-prior-async-audit-v1.json`.
- `rust/target/real-app-read-measurements-unit-v1.log`,
  `rust/target/real-app-read-all-tests-v1.log` and
  `rust/target/real-app-read-tsc-v1.json`.
- `rust/target/real-app-read-handoff-freeze-v1.json`,
  `rust/target/real-app-read-syntax-v1.json`,
  `rust/target/real-app-read-historical-seals-v1.json` and
  `rust/target/real-app-read-verify-fast-v1.log`.
- `rust/target/real-app-read-combined-summary-v1.json`.
