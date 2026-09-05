# Browser probe boundary: measurement and retained refusal — 2026-09-05

> Superseded in part the same day: after the lead took the operator decision on
> the browser pin, ADR 0033 admitted the reserved profile and the three
> census-complete candidates completed it. This report remains the record of the
> measurement and of the seventeen census refusals, which are unchanged. See
> `docs/2026-09-05-controlled-browser-execution.md`.

ADR 0032 was written before any implementation and admits nothing. This report
records the measurement behind it, classifies the seventeen remaining native
census refusals from source, and restates the ordinary three-row baseline.

## Gains, stated separately

| quantity | before this slice | after this slice |
| --- | ---: | ---: |
| candidates loaded under a controlled profile | 20 of 40 TypeScript (23 of 43 with published JS) | unchanged |
| completed mandatory gates | 20 | unchanged |
| real-package contradictions | 0 | 0 |
| scoped accepted `creates` closures | 20 | unchanged |
| ordinary `exportsProven` | 0 | 0 |
| new profile, receipt, protocol, scheme or request identity | — | none |

No certificate was issued, weakened or widened. The slice's product is a
decision with measured premises and one regression pin.

## The measurement

The instrument was the ambient Playwright headless shell (Google Chrome for
Testing 151.0.7922.34, one regular arm64 Mach-O file, SHA-256
`7687bff7cb2db075f250e6d5848bbc8838cac3802ac3952a899c574f8eccab45`, 163,329,264
bytes, 19 sibling resource files, zero symlinks, ad-hoc linker-signed). It was
used **only as an instrument**; ADR 0031's rule that it is not certification
authority is unchanged.

Driver and result:

- `/private/tmp/browser-boundary-measurement/measure.mjs` — SHA-256
  `24bc18d38f1d0c08629c3bf8f8bbb09fb86e6a60d4fe559fb6cef6051264e809`
- `/private/tmp/browser-boundary-measurement/results.json` — SHA-256
  `394f45e39795389c66fc38f81ff35033e3fc6dc0ec30be905208fc335cc769a7`

Inputs were the authenticated Kobalte 0.9.2 sources read out of the pinned
archive (`get-scroll-parent.ts` SHA-256
`b8fe99527dd8b55979c6ffaf927838b880548ed9c118cdcf8e01a743ed58a18b`,
`run-after-transition.ts` SHA-256
`b75871b579280f2e0b6955995172ad7ad8577d175e84a49e7ca063be09da1fbd`), stripped
by pinned Node 24.11.1 `stripTypeScriptTypes` in `strip` mode (derived digests
`966b0d51…f93eb` and `cfe66644…6603a`). The browser transformed nothing.

What one launch established:

| question | answer |
| --- | --- |
| CDP over launcher-owned descriptors 3/4, no port | yes; the whole run took 364 ms |
| a pre-document bootstrap runs before any module | yes, at `readyState === "loading"`; it captured the report binding, deleted the global, froze the three intrinsic prototypes; a recipe-side `defineProperty(Object.prototype, …)` threw |
| every request answerable from memory, unmapped refused | 6 intercepted; 4 fulfilled (document, recipe, two derived modules); `https://example.com/` and a worker script refused |
| browser services the three candidates need | `document`, `getComputedStyle`, `requestAnimationFrame` present; two animation frames fired in headless mode |
| the two candidates behave | `getScrollParent(null) === document.documentElement`; `runAfterTransition` callback invoked after two frames; zero globals added |
| exactly one startup and one run frame | yes, both carrying the launch nonce |
| process group | 5 processes before `killpg`, 0 after |
| the browser's own writes | 51 files in the profile directory during the run |
| storage and workers under the flags used | `localStorage` writes allowed; service-worker registration refused; `new Worker` constructed (its script fetch refused afterwards); network `fetch` refused; `file:` fetch unsupported |

The last two rows are the measured shape of two of ADR 0032's missing
premises: detect-and-refuse cannot cover the profile directory, and the realm
boundary includes worker and storage surfaces whose denial would be the
browser's, not the checker's.

## The seventeen census refusals, read from source

All seventeen are refusals of the independent native `creates` census, before
any probe. None is an observed contradiction — nothing executed — and none is a
resolver or plumbing bug. Each names a possible user-code or host-accessor
invocation the census cannot exclude without a premise this repository has
deliberately not adopted. The independent accessor-census blocker, including
`@solid-primitives/i18n`, remains out of scope; no accessor trust was broadened.

| candidate(s) | refusing form (from the producer) | what the source does | classification |
| --- | --- | --- | --- |
| 0.9.2 `isString` | `CallableFunction.call` | `Object.prototype.toString.call(value)` reads `value[Symbol.toStringTag]`, a getter a caller's object may define | user code reachable through a parameter; whether an accessor rooted at a parameter is the caller's responsibility is an undecided accessor-trust question |
| 0.9.2 `isMac`, `isIPhone`, `isIPad`, `isIOS`, `isAppleDevice` | `property-access-unknown-accessor` at `platform.ts` | `window.navigator.userAgentData?.platform`, `navigator.platform`, `navigator.maxTouchPoints` — host accessors on a global whose `navigator` binding page code can redefine | host-realm integrity premise; not a parameter, so not even the accessor question above |
| 0.9.2 and alpha `isPointInPolygon` | `property-access-unknown-accessor` (element access) at `polygon.ts` | `polygon[i]`, `polygon[j]` and tuple destructuring on a caller-supplied array; a Proxy or index getter runs user code | user code reachable through a parameter; accessor-trust question |
| 0.9.2 `scrollIntoView`, `scrollIntoViewport` | `property-access-unknown-accessor` (element access) at `scroll-into-view.ts` | `child[prop]` with `prop` computed (`offsetLeft`/`offsetTop`) on a caller-supplied element, plus DOM layout accessors | computed member on a parameter and host accessors |
| 0.9.2 `isElementVisible`, `isFocusable`, `isTabbable` | `instanceof` at `tabbable.ts` | `element instanceof HTMLElement`; `HTMLElement[Symbol.hasInstance]` can be defined and `globalThis.HTMLElement` reassigned | user code reachable through the global constructor; host-realm integrity premise |
| 0.9.2 `getAllTabbableIn` | `Array.filter` handed a callable the census cannot see | `elements.filter(isTabbable)` passes a module-local function by reference; following it lands on the `instanceof` refusal above | producer gap ADR 0008 already records, and the callee refuses regardless |
| 0.9.2 `focusWithoutScrolling` | `instanceof` | `parent instanceof HTMLElement`; also a literal `get preventScroll()` the host invokes | as above; host-realm premise |
| 0.9.2 and alpha `debugPolygon` | `get-accessor` at `polygon.ts` | `svg.style.*` setters, `polygonElement.parentElement`, `document.body` — host accessors on DOM objects | host-realm integrity premise |

A browser execution profile changes none of these rows: the census decides the
claim before any probe runs, and a finite probe cannot waive a census refusal.

## Ordinary three-row baseline

No analyzer or certifier code changed in this slice (the only Rust change is a
`#[cfg(test)]` constant and a test assertion), so the ordinary rows are
unchanged by construction. The same recipe corpus was run again for the record;
its outcome is stated in the verification section below. The previous after
file remains `/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/389877ba-d8a8-4f5e-9628-89e210df2471/scratchpad/probe-ts/final/x.json`
(SHA-256 `c3188aa6749b70e3d4c980066638a9a26bafe6327e1adac416da63973626c3af`):

- `@kobalte/utils@0.9.2|solid1|only`: mandatory probe gate
  `sha256:a9c9b71f143168b06eca801102c156b52b0b0302faa47780db2daa51cf88f7bc` did
  not complete;
- `@kobalte/utils@2.0.0-alpha.0|solid2|only`: existing JavaScript cases certify,
  11 source closures withheld;
- `@solid-primitives/i18n@2.2.1|solid1|only`:
  `property-access-unknown-accessor (SpreadAssignment)` at
  `dist/index.js:3471..3483`;
- `exportsProven`: 0.

The checked ecosystem report was produced without the corpus and is not a
baseline for this comparison.

## Certificate applicability and remaining refusals

Nothing new applies. Existing controlled receipts (v5, profiles
`node-strip-inert-esm-v1`, `node-strip-import-free-esm-v1`,
`node-strip-relative-ts-graph-esm-v1`) still apply only to the checker's own
fresh replay; ordinary consumers refuse them. The reserved name
`chromium-headless-shell-cdp-pipe-esm-v1` is refused as an unknown profile.

Remaining refusals across the original 40 TypeScript candidates: 17 native
census refusals (table above) and 3 census-complete candidates awaiting a
browser profile whose six missing premises ADR 0032 names. Solid core remains
`builtin` under ADR 0027, not independently certified.

## Verification

The slice changed one `#[cfg(test)]` constant, one test assertion, and
documentation. Proportional checks:

- `make test-probe-harness`: 102 passed, 0 failed (the relative-graph tracer
  now also asserts the reserved browser profile is refused as unknown);
- the same-corpus three-row runner, fresh output SHA-256
  `3198cc4f5d18f8e5f819cd1e748f6d2e039db702d09247ea21d24d3a5f3d28b0`: every
  row keeps its class and reason — Kobalte 0.9.2 refuses on gate
  `a9c9b71f…f7bc`, the alpha row certifies with 11 withheld closures, i18n
  refuses on demand `3da8a07c…47f1`'s accessor census, `exportsProven` 0 on all
  three;
- `cargo fmt --check`, `git diff --check`, and `cargo clippy -p
  solid-facts-backend --all-targets -- -D warnings` pass;
- the debug checker was rebuilt through `make build-checker-debug` afterwards so
  the compiled Type Facts, harness and Node pins are present.

Intentionally not run: `make verify`, the ecosystem benchmark, coverage,
ownership and the contract corpus (no analyzer, fixture, contract or snapshot
input changed), and any repin of baselines or phase ledgers. No commit or push
was made.
