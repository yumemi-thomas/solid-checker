# Controlled browser execution over a CDP pipe — 2026-09-05

ADR 0033 was written before implementation. It admits
`chromium-headless-shell-cdp-pipe-esm-v1` as a fourth controlled execution
profile: the authenticated `.ts` graph of ADR 0030, still erased by the native
parser and reproduced by pinned Node, executed by a pinned headless-shell
browser that the checker drives over a launcher-owned Chrome DevTools Protocol
pipe. Like every controlled profile it produces no ordinary package contract and
claims no compatibility with an application's browser, bundler or transform.

## Gains, stated separately

| quantity | before | after |
| --- | ---: | ---: |
| candidates loaded under a controlled profile | 20 of 40 TypeScript (23 of 43 with published JS) | **23 of 40** (26 of 43) |
| completed mandatory gates | 20 | **23** |
| real-package contradictions | 0 | 0 |
| scoped accepted `creates` closures | 20 | **23** |
| native census refusals | 17 | 17 (unchanged; a browser cannot waive a census) |
| ordinary `exportsProven` | 0 | 0 |

The three new closures are the three census-complete browser candidates ADR
0031 had left incomplete: Kobalte 0.9.2 and 2.0.0-alpha.0 `getScrollParent`,
and 0.9.2 `runAfterTransition`. Nothing else in the population moved.

## What was built

- **Browser pin.** `PROBE_BROWSER` (optional Makefile variable, real path of the
  headless-shell executable) becomes `SOLID_CHECKER_PROBE_BROWSER_SHA256`, the
  tree digest of the executable's directory in `hash_tree`'s framing, computed
  by `scripts/probe-browser-identity.mjs` and recomputed by the verifier before
  every launch and in every census. A unit test pins the two framings against
  each other on a synthetic bundle, including the symlink refusal.
- **Driver.** `probe_harness/browser.rs`: a checker-owned CDP client over
  descriptors 3 and 4, request-stage interception answering every request from
  memory through an exact URL map or refusing the launch, an import map derived
  from the plan's resolution and the authenticated relative edge map (scoped per
  importer URL), a Rust-authored page bootstrap instantiated per launch with the
  nonce and pid, a CSP on the served document, auto-attach refusal of any new
  target, main-world-only frames, and a private workspace whose only mutable
  subtree is the browser's own profile directory.
- **Transform premise kept.** Pinned Node reproduces every derived module's
  bytes (`stripTypeScriptTypes`, strip mode) before a byte is served; the
  browser transforms nothing.
- **Identities.** Controlled receipt v6 with a new signature domain and proof
  identity `policy-2-with-browser-cdp-pipe-erasure-v1`; a sibling browser
  sandbox scheme (`scheme-version:11`, `scheme-family:browser-cdp-pipe`, its
  30-field vector pinned by a literal test); execution request schema 9 with
  `probeBrowserExecutable`; a bounded `animation-frames` drain step and
  `maxAnimationFrameTurns` recipe policy. The Node scheme-10 vector, every plan
  digest, corpus root and transcript digest computed before this slice are
  byte-identical by construction (each new field is appended only when
  nonzero), which the unchanged contract corpus confirms.
- **Fixture.** `fixtures/package-contracts/probe-source-disposition/browser-only`
  and three recipes; the tracer
  `the_probe_controlled_browser_profile_executes_a_dom_dependent_graph_over_a_cdp_pipe`
  runs only when `PROBE_BROWSER` is set, fails loudly under
  `SOLID_CHECKER_EXPECT_BROWSER_PIN=1`, and asserts: the Node relative-graph
  profile leaves the same graph's gate incomplete (no fake globals); the browser
  profile completes census, veto, receipt authentication, refusal controls and
  replay; and bundle-pin mismatch, missing browser, unreproduced derived
  output, an unmapped module request, and a derived-byte contradiction each
  refuse.

## Production measurement

Driver `/private/tmp/browser-profile-measurement/measure.mjs`; results
`/private/tmp/browser-profile-measurement/results.json`, SHA-256
`ffd3cb27f68f38e7f1e9fd72b24cb4acb6cb1238fc06dcedeea069b67b80e988`. Each case
reuses the authenticated planning of the earlier browser-census diagnostic and
requests a version-9 controlled execution from the freshly pinned debug
checker. Bundle: Google Chrome for Testing 151.0.7922.34, tree digest
`3e0e7118a8bc5993e6d3dbf38bd893332f2ace2c6243872b83b18b532ef0d683`.

| candidate | gate | drain | outcome | wall clock |
| --- | --- | --- | --- | ---: |
| Kobalte 0.9.2 `getScrollParent` | `6fdc49fb…24b3f` | 1 microtask | completed; receipt v6; recipe replayed | 38.2 s |
| Kobalte 2.0.0-alpha.0 `getScrollParent` | `7b0eb83d…4217` | 1 microtask | completed; receipt v6; recipe replayed | 36.5 s |
| Kobalte 0.9.2 `runAfterTransition` | `b11d2416…3eac4` | 2 animation frames | completed; callback ran within two frames; receipt v6; recipe replayed | 36.8 s |

`runAfterTransition`'s module initialization registered its `transitionrun` /
`transitionend` listeners on a real `document.body`; no global was added by
either export, so the `create-operation` veto marker never fired. The wall
clock is dominated by five tree censuses of the 196 MB bundle per launch pair
and by the Type Facts census; it is recorded, not optimized.

## Certificate applicability and remaining refusals

The receipt means that the native census closed `creates` over authenticated
source, pinned Node reproduced the derived bytes, and the veto saw no
contradiction while exactly those bytes ran in the pinned headless shell
through the checker's URL map. Its only consumer is the checker's own replay;
ordinary policy-2 consumers refuse it, as they refuse every controlled receipt.
It says nothing about an application's browser or toolchain, and Vitest browser
mode remains what ADR 0031 said it is: not authority.

The 17 native census refusals stand as classified in the browser-boundary
measurement report; none is affected by an execution profile. Solid core
remains `builtin` under ADR 0027. The ordinary three-row baseline is unchanged
by construction and its rerun is recorded below.

## Verification

All checks ran against the debug checker rebuilt through
`make build-checker-debug PROBE_BROWSER=…`, so the compiled Type Facts, harness,
Node and browser pins were present:

- backend library suite with all pins: 384 passed (the browser tracer, the
  browser policy-literal test and the bundle-identity parity test included);
- `make test-probe-harness PROBE_BROWSER=…`: the 105 probe-filtered tests,
  including the Node-profile tracers unchanged;
- armed process suites: contracts 11, diagnostics 15, dialects 37 passed;
- contract corpus, non-updating: 94 fixtures compared, nothing stale — every
  pre-existing plan digest, corpus root and receipt binding is byte-identical;
- coverage: 94 projects, 547 findings, no moves;
- ecosystem and scripts Vitest suites: 406 tests in 39 files;
- phase19 audit: 185 stable mains, unchanged (the browser fixture is a tracer
  input with no main document);
- workspace Clippy with `-D warnings`, `cargo fmt --check`, `git diff --check`,
  the reactivity schema and both dialect manifests;
- the same-corpus ordinary three-row runner, rerun after the final rebuild; its
  outcome is stated in the closing section below.

Intentionally not run: `make verify`, the ecosystem benchmark, the ownership
gate (no finding or dialect input changed), the CLI test suite (no
`packages/cli` file changed; the harness manifest is byte-identical), and any
repin of baselines or phase ledgers. No commit or push was made. Building
without `PROBE_BROWSER` — the default, and CI's situation — compiles a verifier
that refuses the browser profile by name and skips its tracer; that is the
stated limit, closed only on a machine that names a browser.

## Ordinary three-row baseline

The same checked recipe corpus was run against the final rebuilt checker
(`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/1ea56a4b-1438-48b4-8413-f4a008271f5f/scratchpad/three-row-final/after.json`,
SHA-256 `d8f0484796f3c5656ca20c85ca35e3c3539588c5b9c733c461088000043f643b`).
Every row keeps its class and reason, as expected: the browser profile is a
separate controlled transaction and cannot publish an accepted catalog.

| row | outcome | withheld closures | `exportsProven` |
| --- | --- | ---: | ---: |
| `@kobalte/utils@0.9.2\|solid1\|only` | mandatory probe gate `a9c9b71f…f7bc` did not complete | 0 | 0 |
| `@kobalte/utils@2.0.0-alpha.0\|solid2\|only` | certified; existing JavaScript closures | 11 | 0 |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | independent accessor-census refusal | 0 | 0 |
