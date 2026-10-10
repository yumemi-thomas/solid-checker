# Installed async package source and typing boundaries

## Result

The research feedback path now admits installed ES module async bodies alongside
the application's TypeScript program. The application's original program remains
the gate for published typing errors. Package implementation facts come from a
separate program and an issued revision that records both views. This requires
no new package-specific behavior rule or certified contract.

Cross-file callbacks expose a useful gain: **7/10 stale-result targets receive
hints, compared with 1/10 before package-source instrumentation**. Six newly
detected cases cross the published `retry` implementation's await boundary.
They cover timer delays, rejected Promises, async callbacks, callback options,
namespace imports and repeated attempts. The package is retained published
`@solid-primitives/promise@2.0.0-next.2`, with audited Solid rc.9.

The first inline-callback population already had 10/10 target detections. Its
callbacks themselves supplied admitted read sites, so it establishes no new
target gain. Both populations expose noise from reads discarded before returning
9. Quiet controls worsen from 13/14 to 12/14 in each population: terminal
rejection noise disappears, while two constant-result shapes become noisy.

| Population | Earlier hints | Current hints | Current quiet controls | Typing exclusions |
| --- | --- | --- | --- | --- |
| 26 inline callback variants | 10/10 | 10/10 | 12/14 | 2 |
| 26 cross-file callback variants | 1/10 | 7/10 | 12/14 | 2 |
| Earlier 40 result-flow variants | 8/9 | 8/9 | 25/29 | 2 |
| Earlier 36 continuation variants | 10/15 | 10/15 | 19/19 | 2 |
| Combined 128 stages/variants | — | 35/44 | 68/76 | 8 |

All **128 plain/observed comparisons** preserve tested displayed values, getter
contexts, invocation counts, published typing errors, caught errors and native
diagnostic deliveries. There are no page errors in completed runs. The 76 earlier
variants retain their previous scores; the older 77 revision stages and ten
reference variants are not replayed in this slice. These paired variations reuse
setup and are not independent application defects or package coverage rates.

The two new populations were authored after source profile V1 froze. Discovery
was then refined after real-app preflight failures. Final V2 runs are adapted
replays of those populations, not fresh validation of the refined profile.
Their target/control scores match the original V1 runs. Historical modules,
seals, observations and failed attempts remain unchanged.

Feedback stays informational `intent-open`, with `authority: false`,
`certification: false` and open static dispatch. Execution provenance does not
prove developer intent, general result flow or package-wide behavior.

## Two views, one observation revision

`package-source-session-v2.mjs` wraps the earlier project session. Its published
program uses the actual project configuration and installed declarations. That
program alone determines application typing errors. Adding JavaScript bodies
does not replace or broaden a published signature. Focused checks confirm that
the consumer's `retry` declaration remains the actual `dist/index.d.ts` symbol.

The implementation program additionally admits recorded package source using
`allowJs`, `checkJs: false` and `noEmit`. Its diagnostics are recorded separately.
If they invalidate an otherwise clean published program, implementation facts
are refused explicitly. Published errors still close instrumentation for the
whole input. The extra program is a source-fact view, not a substitute typing gate.

Both programs, metadata, source bytes, configurations, resolution reads/misses,
directory listings and realpaths belong to the observation revision. Source is
discovered before serving a consumer, so serving its package does not issue a
different revision. Foreign, retired and changed-input revisions are refused.
External transformation requires exact recorded enrollment and served bytes.

Plugin V18 reuses continuation selector/transform V2, runtime V5 and projector
V13. Exact helper/operation declarations, mapped function entry and native read
frames, matching current revisions and explicit primitive normal completion
remain required. No observer/owner is restored, await is added or Promise handler
is attached by the continuation instrumentation. The fixed native hook still
requires audited rc.9 bytes; native framework packages are a separate boundary.

## Discovery beyond a small fixture

Initial discovery seeded every external file in the typing program and followed
all dependency/peer metadata. In **all eight analysable retained configurations**
it exhausted its discovery budget and discarded implementation enrollment.
Restricting the initial seeds alone still reached tooling through package peers.

The refined profile seeds exact runtime import/re-export declarations in the
configured application's `src` files. Type-only imports, declaration files and
configuration outside that served source scope do not seed it. Literal dynamic
imports can resolve; computed imports and CommonJS imports stay explicitly open.
Dependencies are followed through parsed module imports and TypeScript resolution,
instead of treating every metadata dependency as executed behavior.

Budgets remain 64 discovered packages, 512 scanned files and 4 MiB of source
parsed during discovery. They do not bound the reconstructed TypeScript program
or runtime buffers. Oversized or unvisited modules stay unadmitted with recorded reasons.
Already admitted exact modules remain available. A partial discovery result
grants no behavior to omitted modules and cannot establish complete package
coverage. Additional implementation-program diagnostics still close that view
when the public program is clean.

A preflight over the same **15 retained publications** now enrolls **49 async
bodies in 23 modules across 14 packages**, with all published typing programs
clean and no implementation-program diagnostics. The earlier syntax inventory
contains 53 async bodies; four are generators outside this profile. All three
bodies in `@solid-primitives/async` are generators, as is one websocket body.
These counts establish enrollment, not reachable calls or successful feedback.

The same real-app preflight admits partial implementation source in **7/8**
analysable configurations, comprising 241 modules. This includes **4/5** typing
clean configurations. Three configurations have existing public typing errors
and remain closed for instrumentation. A ninth configuration lacks the installed
`@solidjs/web` artifact and is refused. No package installation is attempted.

Every one of the eight configurations still reaches a discovery boundary.
One clean configuration admits no async source within that budget. The real apps
are not executed here, so these results establish loader progress only. Cold
preflight takes roughly 1.36–6.77 seconds per configuration; the source layer
accounts for about 0.77–3.55 seconds. Large-app latency and coverage remain open.

In 48 small valid browser fixtures, median source-layer build time is about
226 ms, excluding the original published-program build. Total source validation
time over each fixture's uses has a median of about 57 ms. Four browser processes
ran concurrently. These are fixture measurements, not editor response-time or
large-project performance guarantees.

## Remaining misses and noise

The three new cross-file targets stay missed:

- An object result records an entered helper, then refuses nonprimitive completion.
- A sequential queued task receives no entered continuation tied to the consumer.
- The concurrent queue variant has the same missing attribution. The queue was
  already draining when the task was submitted; callback member operations also
  lack admitted declarations. Its async bodies have implicit completion.

Simply loading a queue's async source does not prove task registration and later
execution belong to the consumer. Both queue cases use retained published
`@solid-primitives/queue@1.0.0-next.3`, unchanged real declarations and a correctly
updating capture control. Their reads actually occur without owner/observer.

Four new control variants remain noisy: inline and cross-file versions of a
synchronous or async callback that discards the read and returns 9 through
`retry`. The current constant proof cannot propagate that callback result across
the published declaration/implementation boundary. Source enrollment itself
does not authorize such a suppression.

The earlier receiver-call, adopted-Promise, nested-reaction, object, generator
and pending-ticket-budget misses remain, as do four earlier result-flow noises.
CommonJS, computed/optional dispatch, unknown module imports, skipped source,
mixed shortcut/async transforms in one module, general intent/result flow,
stateful HMR and automatic installed-dependency updates remain open. Raw source
with dependency prebundling disabled is the tested browser profile.

## Closed-runner memory and verification

Both first broad replays exhaust Node's default heap after about 26 applications.
Browser V10 measures collection after close; collection alone still reaches
roughly 4 GiB and fails. These unfinished runs are excluded from scoring.

An isolated 12-session check releases every old public/source program through
weak references and remains around 29–31 MB after collection. Browser V11 then
retires analysis sessions after closing each server and releases its last program
reference. It completes the 40- and 36-case replays. V12 preserves that lifecycle.
The final collected peaks are below 352 MB. Retained report/evidence memory still
grows across applications; this is not an unbounded-session memory guarantee.

Independent browser audits reconstruct published and implementation programs,
authenticate source/metadata and input manifests, check exact declarations and
mapped continuation frames, and compare plain behavior. They cover **70
observations**, including **59 async observations**, **64 invocation links**
and **12 links inside installed package source**, plus 20 prior suppressions.
The auditor imports no session, selector, transform or projector. It does not
independently reproduce discovery completeness; native models/traces remain
research premises. A separate preflight audit authenticates 67,748 manifest
entries and 264 source modules and reconstructs 23 real typing programs.

Verification passes:

- **519/519** prototype tests, no skips, including 11 original source-session
  checks and 17 refined discovery/typing/revision checks.
- **424** module syntax checks; unchanged modules retain their earlier check.
- **32** historical/current seals authenticate **427** distinct pins unchanged.
- Seventeen explicit `tsc --noEmit` checks: thirteen clean and four expected
  `TS2769`/`TS2322` exclusions. All eight scored typing exclusions stay silent.
- `make verify-fast`, schema JSON validation, dialect manifest validation and
  whitespace checks pass. The producer stamp matches; formatting and workspace
  Clippy run with certification pins.

Initial test attempts retain a failed process, an incomplete build-config fixture
and a missing web import in a serializer test. The corrected setup passes the
focused checks. No published declaration stub is introduced or broadened.

No production Rust, compiler lowering, contract, schema, manifest or finding
snapshot changes. Generated research reports, isolated inputs and seals live
under `rust/target`. Full production verification, fixture coverage/ownership,
contract corpus and certification gates are deferred for this research slice.

## Evidence and reproduction

The [combined summary](../../../rust/target/package-source-combined-summary-v1.json)
links the four final browser audits and original baselines. Additional evidence:

- [Publication enrollment](../../../rust/target/package-source-enrollment-v2.json),
  [real configurations](../../../rust/target/package-source-real-app-enrollment-v3.json)
  and [independent preflight audit](../../../rust/target/package-source-preflight-audit-v1.json).
- [Cross-file gaps](../../../rust/target/package-source-detached-gaps-v2.json),
  [small-fixture cost](../../../rust/target/package-source-cost-v2.json),
  [closed-runner memory](../../../rust/target/package-source-browser-memory-v1.json)
  and [isolated session release](../../../rust/target/package-source-session-memory-v1.json).
- [Tests](../../../rust/target/package-source-all-tests-v2.log),
  [fast checks](../../../rust/target/package-source-verify-fast-v1.log),
  [syntax](../../../rust/target/package-source-syntax-v3.json) and
  [immutable pins](../../../rust/target/package-source-historical-seals-v3.json).

Use fresh output paths. Browser V12 accepts a case module, seal, output directory,
Chromium executable and `reads` or `plain`; it serves local sources only. For the
final source profile use `package-source-detector-freeze-v4.json`. Explicit
collection in a multi-application replay requires Node's `--expose-gc` flag.
Audit V8 accepts the case module, reads report, plain report and a fresh output
JSON. Prior V1 results and all failed V9/V10 replays remain available.
