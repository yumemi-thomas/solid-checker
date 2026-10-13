# Lightpanda compatibility and speed

## Outcome

Lightpanda 1.0.0 runs the eight focused Solid queue cases and passes the
independent source/trace audits. In two corrected sequential timing pairs it
averages **15.53 seconds**, compared with **16.51 seconds** for Chromium. The
observed reduction is **5.9%**, roughly one second per eight-case browser cycle.

| Full browser process, including shutdown | First pair | Reversed pair | Mean |
| --- | ---: | ---: | ---: |
| Chromium | 16.33 s | 16.69 s | 16.51 s |
| Lightpanda | 15.44 s | 15.63 s | 15.53 s |

The order is Chromium → Lightpanda, then Lightpanda → Chromium. No audits or
other benchmark workloads run while these browser processes are timed. The
sample is small and does not establish general speed, memory or compatibility
claims. Browser execution is only part of the complete development cycle.

The observed gain is too small to justify switching the default browser. The
adapter remains available for further experiments; Chromium remains the final
validation browser and the existing development command's default.

## What matched

The unchanged eight cases cover the four formerly missed adopted-child/reaction
targets, two named-data controls and two identity controls. Both engines produce
four target hints, two quiet controls and the same two remaining noisy identity
controls. Changing the browser does not improve warning precision here.

All recorded groups match:

- original source/helper hashes and published typing results;
- initial/final values, visible text, task/getter counts and owner/observer contexts;
- application diagnostics, errors and continuation gaps;
- exact native reader, registered callback, helper and operation source facts;
- normal-body return grades and open Promise-settlement/result-flow claims;
- admitted hints and independently checked named-data suppressions;
- original mapped read, reader, creation, registration, invocation and entry frames;
- recorded runtime entry, completion, candidate and event counts.

Comparison normalizes only the generated case-directory prefix, local origin
port and issued project revisions, which each independent audit checks on its
own. Unmapped null entries are excluded from the mapped-frame lists. Exact
mapped locations, source spans, source hashes and counts remain equal. There
are 24/24 matching case comparisons against the first corrected Chromium run.

Across the initial pilot, preliminary repeats and corrected repeats, all ten
browser runs and their independent audits pass: 80 executions of the same eight
cases. These are repetitions, not 80 independent cases or fresh challenges.

## Artifact and adapter

The official macOS arm64 binary comes from the
[1.0.0 release](https://github.com/lightpanda-io/browser/releases/tag/1.0.0),
published on 2026-10-02. The downloaded bytes match its GitHub asset digest:

```text
sha256:955440053a84754dd64c62f970449a56a2b350cdf43ea5f2e809a73047b8173d
```

It lives under ignored `rust/target/lightpanda-1.0.0/`; no project dependency or
global installation changes. The actual executable reports `1.0.0`. Its CDP
version string is `124.0.6367.29`; that string is not treated as the engine's
identity. The comparison records the actual binary path, version and digest.
Chromium reports `151.0.7922.34` and its executable is pinned separately.

`experiment-browser-engine-v2.mjs` starts an owned loopback CDP server, connects
the retained Playwright client, enables external stylesheet loading, disables
Lightpanda telemetry and stops only the process it created. The original
context isolation, request blocking, application source, waits, source mapping,
runtime hooks and feedback projector remain unchanged.

Browser V34 imports that adapter. It accepts `chromium` or `lightpanda` as the
last argument after the selected-case JSON. Its source/profile and executable
are authenticated. Independent audit V19 validates each run against the
unchanged authenticated full Chromium plain baseline. It does not infer
Lightpanda compatibility from using V8 or exposing CDP.

## Preliminary timing correction

The first adapter used referenced two-second timeout promises around successful
shutdown. Even when cleanup finished promptly, those timers kept Node alive.
Preliminary total times therefore made Lightpanda look slower. Those reports
remain preserved under `lightpanda-benchmark-v1` and are excluded from the final
speed claim. Their compatibility observations still match.

Adapter V2 makes the deadlines unreferenced. A real child-process regression
checks that successful owned-process cleanup does not keep Node alive for two
seconds; it passes in approximately 58 ms. The corrected benchmark measures
the complete child-process lifetime, including browser/server shutdown, rather
than relying only on a report written before cleanup.

The corrected mean navigation phases are 10.06 seconds for Chromium and 9.77
seconds for Lightpanda. Application transforms, readiness waits, source/type
analysis, feedback projection and independent audits remain substantial costs.
This experiment does not change those phases.

## Reproduction and validation

Run from the repository root with fresh output paths:

```sh
node benchmarks/reviewed-package-models/lightpanda-benchmark-v2.mjs \
  rust/target/lightpanda-benchmark-N
node benchmarks/reviewed-package-models/lightpanda-comparison-v2.mjs \
  rust/target/lightpanda-benchmark-N/results.json \
  rust/target/lightpanda-comparison-N.json
node --test benchmarks/reviewed-package-models/experiment-browser-engine-v2.test.mjs
```

The saved final evidence is `rust/target/lightpanda-comparison-v2.json` with
`lightpanda-benchmark-v2/results.json`, four browser reports and four audits.
The earlier two pilot reports and four preliminary repeats remain separate.
All feedback and reports have `authority: false` and `certification: false`.

The previous 627 code/input pins remain unchanged. Source syntax, shutdown
regression, independent browser audits, comparison authentication, universal
fast checks, schema, dialect manifests and whitespace pass. Full production
coverage, ownership, certification, performance and release gates remain
deferred for research-only browser tooling. No production rule, fixture
snapshot, contract, dependency manifest or compiler pin changes.

General package support, layout-dependent code, hydration, navigation/HMR,
resource observers, long sessions, memory use and the full 58-case population
under Lightpanda remain unmeasured. The result supports optional use for this
focused population; final broader validation continues to use Chromium.
