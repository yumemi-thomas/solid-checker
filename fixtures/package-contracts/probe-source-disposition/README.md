# probe-source-disposition

ADR 0009's retained refusal, exercised by
`contract_certification::tests::the_probe_gate_tracer_keeps_typescript_source_incomplete_and_javascript_certifiable`.
This is a certification tracer, not a generator corpus fixture; it has no
checked-in main contract or snapshot. The test constructs exact archives and
one hand-authored `creates: []` candidate per package, then acquires the real
pinned producer's census before running the real pinned Node/harness.

| package | only runtime artifact case | outcome |
| --- | --- | --- |
| `probe-typescript-source-only`, ordinary consumer | `index.ts` | census passes; Node load fails; `IncompleteGate` names the scheduled gate; closure refuses |
| `probe-typescript-source-only`, controlled consumer | `index.ts` | `node-strip-import-free-esm-v1`: the typed identity function completes census, exact erasure, derived veto, scoped receipt authentication and recipe replay; ordinary consumers refuse the receipt |
| `probe-published-javascript` | `index.js` | census passes; recipe completes; closure certifies with a nonempty gate root |
| `probe-browser-source-only` (`browser-only/`), Node relative-graph profile | `index.ts` → `./dom` | census passes; the recipe's `document` reference is a recorded run error; `IncompleteGate` — no fake globals |
| `probe-browser-source-only`, controlled browser consumer | `index.ts` → `./dom` | `chromium-headless-shell-cdp-pipe-esm-v1` (ADR 0033): pinned Node reproduces both derived modules, the pinned headless shell executes exactly the served URL map, one `animation-frames` drain turn elapses, the scoped receipt (version 6) authenticates and the recipe replays; bundle-pin, missing-browser, derived-output, unmapped-request and derived-contradiction controls each refuse |

The TypeScript source is a typed identity function; the JavaScript sibling is
an ordinary no-op control. Each package publishes its own manifest and
declarations, with one `.` runtime case and no dependency.
There is no fallback `.js` artifact in the TypeScript package. The recipes
statically import the exact package, call `noop`, and emit a call enter/exit
pair; a changed return emits the contradiction marker. Neither passes the
session or harness into the package. This bounded observation never proves
absence; the census proves the closure.

The test checks a recorded worker error as well as the typed `IncompleteGate`:
a timeout, missing frame or resolution mismatch cannot satisfy the negative
arm. The worker hashes error details, so this does not assert the error text;
the real-package reproduction recorded in ADR 0009 identifies the load cause.
The positive
arm verifies receipt authentication and the closed claim in the canonical main.
The normal compiled-in pins are used; missing pins fail loudly under the
Makefile's `SOLID_CHECKER_EXPECT_PROBE_PINS=1`.

`browser-only/` is ADR 0033's tracer input, exercised by
`the_probe_controlled_browser_profile_executes_a_dom_dependent_graph_over_a_cdp_pipe`
only when `PROBE_BROWSER` names a pinned headless-shell executable; without one
the test skips (and fails loudly under `SOLID_CHECKER_EXPECT_BROWSER_PIN=1`).
Its `scrollRoot` mirrors Kobalte's `getScrollParent` fallback: `dom.ts` reads
`document.scrollingElement || document.documentElement`. The recipe compares the
result with `document.documentElement`; `browser-unmapped.mjs` imports a module
URL outside the served map and must refuse the launch (a `fetch` is rejected
earlier by the browser-enforced `connect-src 'none'`); `browser-derived-veto.mjs`
reflects the derived source and emits the contradiction when the return
annotation is blanked, a veto control rather than a real defect.

ADRs 0028 and 0030 use the strengthened controlled consumer with a pinned
strip-only exact-URL override, sandbox scheme 10 and worker protocol v5. Ordinary certification
retains ADR 0009's refusal and never installs this hook. The regression also
checks gate-time source/output/retained-output/Node-pin mismatches, receipt
mutation, and a deliberate veto after observing derived function spelling.
`restricted-type-erasure` retains the reflection counterexample and
unsupported-profile rejection at the active ordinary receipt consumer. The failure
is a certification/load disposition, not a diagnostic against a TypeScript
consumer. No Solid typings are stubbed.
