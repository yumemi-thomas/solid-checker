# ADR 0032: A pinned browser probe boundary — measured design, admission withheld

- Status: accepted; the reserved profile was subsequently admitted by ADR 0033
  (2026-09-05) after the lead took the operator decision on the browser pin and
  the two policy decisions named below were taken in that ADR
- Date: 2026-09-05
- Owners: package-contract certifier and probe harness
- Relation: makes ADR 0031's requirement list concrete. ADR 0031's refusal
  remains in force; this ADR names what a browser profile would be and the
  exact premises it still lacks.

## Context

Three original TypeScript candidates pass the independent native `creates`
census and reach a mandatory gate that no execution profile can complete:
`getScrollParent` in Kobalte 0.9.2 and 2.0.0-alpha.0 (identical source), and
`runAfterTransition` in 0.9.2. Their bodies are ordinary browser code:

- `getScrollParent(node)` walks `parentElement` while
  `window.getComputedStyle(node)` reports no `auto|scroll` overflow, and falls
  back to `document.scrollingElement || document.documentElement`;
- `runAfterTransition(fn)` calls `requestAnimationFrame` and, at module
  initialization, registers `transitionrun`/`transitionend` listeners on
  `document.body` when `document.readyState !== "loading"`.

ADR 0031 refused to synthesize `document`, `window`, layout or animation frames
in the Node worker and listed six properties a browser profile would have to
bind. It did not say whether any real browser boundary *can* bind them. This
ADR answers that with a measurement rather than an argument.

## Measurement

The instrument was the ambient Playwright headless shell on this machine —
`chrome-headless-shell`, Google Chrome for Testing 151.0.7922.34, one regular
arm64 Mach-O file of 163,329,264 bytes, SHA-256
`7687bff7cb2db075f250e6d5848bbc8838cac3802ac3952a899c574f8eccab45`, beside 19
resource files (ICU data, V8 context snapshot, `.pak` bundles, ANGLE and
SwiftShader libraries) and **zero symlinks**. It is ad-hoc linker-signed and its
only provenance is a Playwright revision number. **It is an instrument for this
measurement and nothing else**: ADR 0031's prohibition on treating it as
certification authority is unchanged, no receipt was issued, and no repository
code or pin references it.

A 150-line driver (`/private/tmp/browser-boundary-measurement/measure.mjs`,
SHA-256 `24bc18d38f1d0c08629c3bf8f8bbb09fb86e6a60d4fe559fb6cef6051264e809`)
launched the shell once, with results at
`/private/tmp/browser-boundary-measurement/results.json` (SHA-256
`394f45e39795389c66fc38f81ff35033e3fc6dc0ec30be905208fc335cc769a7`). The
derived module bytes were pinned Node 24.11.1's `stripTypeScriptTypes` output of
the authenticated 0.9.2 sources; the browser transformed nothing.

| ADR 0031 requirement | mechanism measured | result |
| --- | --- | --- |
| driver and transport identity | `--remote-debugging-pipe`: Chrome DevTools Protocol over launcher-owned descriptors 3 (in) and 4 (out), no debugging port, no npm driver | works; whole run 364 ms; `Browser.getVersion` reports product, revision and V8 version |
| checker-owned bootstrap before any package code | `Page.addScriptToEvaluateOnNewDocument` | ran at `document.readyState === "loading"`, before the import map or any module; captured the `Runtime.addBinding` function and deleted it from `window`; froze `Object`, `Array` and `Function` prototypes — a recipe-side `defineProperty(Object.prototype, "toJSON")` threw |
| exact source/derived graph and module resolution | `Fetch.enable` at request stage on every URL; a synthetic `https://solid-checker.invalid` origin; an import map from bare specifiers to exact synthetic URLs; every response served from memory | 6 requests intercepted; 4 fulfilled (document, recipe, two derived modules); 2 unmapped requests refused (`https://example.com/` and a worker script); no filesystem or network read |
| bounded startup/run reporting | one startup and one run frame through the captured binding, each carrying the launch nonce | exactly two frames observed; `runAfterTransition` callback ran after two animation-frame turns; `getScrollParent(null) === document.documentElement`; zero globals added |
| process-group termination | `process_group` launch, `killpg(SIGKILL)` | 5 processes in the group before the kill; 0 survivors after |
| browser services present | `typeof document/getComputedStyle/requestAnimationFrame` | all present; `requestAnimationFrame` fires in headless mode |
| filesystem policy | `--user-data-dir` inside the private directory | **51 files written by the browser itself during one 364 ms run** (`Cache`, `Code Cache`, `Cookies`, `Local Storage`, `Service Worker`, `Session Storage`, `Shared Dictionary`, `WebStorage`, `blob_storage`, …) |
| storage / workers / network dispositions | flags `--disable-features=ServiceWorker`, `--host-resolver-rules=MAP * ~NOTFOUND`, downloads denied | service-worker registration refused; `fetch` to the network refused; `file:` fetch unsupported; **`localStorage` writes allowed**; **`new Worker("/w.js")` constructed** (its script fetch was then refused, but a `blob:`/`data:` worker would need no fetch) |

## Decision

**Retain ADR 0031's refusal. Admit no browser profile.** No receipt version,
worker protocol, sandbox scheme, execution-request version or policy field
changes; nothing about wire meaning changes, so no identity is bumped.

Reserve the profile name `chromium-headless-shell-cdp-pipe-esm-v1` for the
design below, and pin that today it is refused as an unknown controlled
execution profile before any census, probe or receipt
(`RESERVED_BROWSER_EXECUTION_PROFILE` in `controlled_execution.rs`, asserted by
the relative-graph tracer). A request naming it fails the version/profile
dispatch exactly as any unknown profile does.

### The design the reservation names

If admitted, the profile would be:

1. **Pin.** `SOLID_CHECKER_PROBE_BROWSER_SHA256`, computed by the build from an
   operator-supplied `PROBE_BROWSER` directory as a path-ordered tree digest
   over its regular files, refusing symlinks and any non-regular entry
   (`hash_tree` discipline). Verified before every launch and re-asserted in
   every census against the pin, as `node-executable` is. The version is asked
   of the pinned bytes (`--version`) and must be echoed by `Browser.getVersion`
   and by the startup frame's user agent.
2. **Driver.** A checker-owned CDP client in the verifier over the pipe. The
   page bootstrap is one more member of the harness source manifest. No
   Playwright, Puppeteer or Vitest code is on the report path or in the pin.
3. **Module supply.** No filesystem or network origin. Every request is
   intercepted at request stage and either fulfilled from the authenticated
   derived bytes keyed by exact URL under a fixed synthetic origin, or failed.
   The import map is derived from the plan's verified resolution and the native
   relative edge map (ADR 0030). The served document carries a CSP that names
   the origin for `script-src` and sets `worker-src`, `child-src`, `frame-src`,
   `connect-src`, `object-src` to `'none'`, with no inline, `blob:` or `data:`
   sources.
4. **Bootstrap.** Before the document: capture the report binding and every
   primordial the report path needs, freeze the three prototypes, then import
   the recipe after `DOMContentLoaded` and hand it the harness. Exactly one
   startup and one run frame; a second startup frame (an iframe re-running the
   bootstrap) is a protocol refusal; `Target.setAutoAttach` refuses new
   targets.
5. **Lifecycle.** Own process group, `killpg` on every exit path, bounded
   startup and run budgets, `env_clear` plus the same allowlist.
6. **Policy flags.** Profile directory inside the private tree, no first run,
   background networking, component update, sync, crash reporting and GPU
   caches disabled, `--host-resolver-rules=MAP * ~NOTFOUND`, downloads denied,
   permissions denied, an explicit storage disposition.
7. **Consumer.** The checker's own fresh replay under the same pins, exactly
   as ADRs 0028 and 0030: receipt v6 with its own signature domain and proof
   identity, a browser report protocol identity, sandbox scheme 11, execution
   request schema 9. Ordinary policy-2 consumers refuse it.
8. **Drain.** A new bounded `animation-frames` drain step in the recipe policy.

### Why admission is withheld: the concrete missing premises

Each item below is a decision or an input that does not exist in this
repository. None is an engineering detail the measurement resolved.

1. **There is no browser build input.** The Makefile pins `PROBE_NODE` only.
   There is no `PROBE_BROWSER`, no compiled digest, no acquisition or CI
   provisioning path, and Chrome for Testing and Playwright distributions
   publish no checksum manifest comparable to nodejs.org's `SHASUMS256.txt`.
   The only browser on this machine is Playwright's cache — ad-hoc signed,
   identified by a Playwright revision — and ADR 0031 forbids treating it as
   authority. Pinning bytes with no upstream provenance is a decision the
   operator has to take, and acquiring them needs the network this slice may
   not use.
2. **Detect-and-refuse cannot cover the browser's own writes.** 51 files
   landed in the profile directory during one run. A browser profile needs
   either a *carve-out* — a mutable subtree inside the private directory that
   the census does not watch, which changes what `watched:private-directory-entries`
   means — or a move from detection to *denial* resting on Chromium's renderer
   sandbox and CSP, which ADR 0006 reserves for Stage 2 because it changes the
   proof-policy digest. Either is a scheme-meaning change to review, not a
   field to add.
3. **The resolution-independence premise has no browser analogue.** ADR 0025
   requires the recipe's resolution echo to equal the selected target and
   states that output identity does not replace `verify_reported_resolution`.
   A browser has no package resolver and applies no export conditions; under
   this design the checker's import map *is* the resolver, so the echo would
   only confirm the checker's own map. The `refuse_unreproducible_artifact_case`
   and `observe_conditions` checks are meaningless there. A reviewed
   replacement premise ("checker-authored exact URL map; no interpreter
   resolver; conditions inapplicable") is required and has not been decided.
4. **The realm boundary is wider than one realm, and its denial is the
   instrument's.** A `blob:` or `data:` dedicated worker runs outside the
   frozen realm with nothing to intercept; iframes create fresh realms; helper
   processes sit outside every JavaScript guarantee. CSP and auto-attach can
   deny these, but the denial is enforced by the pinned browser and not
   verified by the checker. Every such step needs a disposition row like ADR
   0006's resolver table before the policy digest can name it, and none is
   written.
5. **The interpretation is a hybrid.** The browser strips no types; the derived
   bytes remain pinned Node's strip-only output. The certificate would mean
   "Node-strip bytes executed by pinned Chromium through a checker import map"
   — an interpretation no application performs. It is scoped and statable, but
   it is a third derived profile whose reflection hazards (ADR 0028) must be
   re-pinned.
6. **Animation-frame drains change the evaluator's determinism premise.** The
   repeat-run comparison must hold across frame timing; the measurement shows
   two deterministic frames, not a policy.

## What this buys and does not buy

If every premise above were established, the profile would advance at most the
three census-complete candidates to scoped `creates` closures under a receipt
that no ordinary consumer accepts. `exportsProven` would remain 0. The other
three browser-feasibility samples (`focusWithoutScrolling`, both `debugPolygon`)
and the fourteen non-browser census refusals are proof refusals in the native
census — user-code or host-accessor invocations the census cannot exclude — and
a browser executor cannot waive one of them. The accompanying dated report
classifies all seventeen from source.

## Alternatives considered

- **Vitest browser mode, Playwright or Puppeteer as the driver.** Their
  transform, resolver, page bootstrap and driver closure are not in any pin,
  and the report path would traverse code the checker never hashed. Rejected
  (ADR 0031).
- **The full Chrome for Testing `.app`.** Its framework bundle carries five
  symlinks (`Versions/Current` and friends), which `hash_tree` refuses by
  design; the headless shell is the only bundle shape the existing census
  discipline could pin without a new symlink rule.
- **Serving modules from `file:` URLs or a local HTTP server.** Both add an
  origin the checker does not own — the filesystem or a socket — where
  interception from memory adds none. Rejected in favour of request-stage
  fulfilment.
- **Fake browser globals in the Node worker.** Tests the shim. Rejected (ADR
  0031).
- **A census-only certificate with the veto withheld.** A weaker,
  consumer-visible class the current consumer could mistake for a vetoed row.
  Rejected (ADRs 0028 and 0030).

## Consequences

- The three census-ready browser candidates remain incomplete. This is a
  harness/profile blocker with a measured design and six named premises, not
  evidence about their `creates` domains.
- Nothing certifiable changes: no receipt, snapshot, contract, protocol or
  policy identity moves; the reserved profile name is refused by test.
- Revisiting requires, in order: an operator decision on a provenance-bearing
  browser build input; a reviewed sandbox-scheme decision on the profile
  directory (carve-out or denial); a reviewed replacement for the resolution
  premise; a disposition table for the browser's own service surface. Only then
  a new ADR, receipt v6, scheme 11 and request schema 9.
