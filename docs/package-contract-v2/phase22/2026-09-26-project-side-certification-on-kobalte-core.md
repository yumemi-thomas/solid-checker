# Project-side certification on a real consumer: kobalte core

Measured 2026-09-26 at c4ab5c54, with a frozen copy of the release binary. The
consumer is `kobaltedev/kobalte` `solid2` e9d426d4 (pnpm; solid-js, web and
signals rc.3), analysed through `packages/core/tsconfig.json`. The baseline is
the same binary and tier without a project catalog. Raw data and scripts live
in the session scratchpad (`certify-kobalte/`) and are not checked in. **[M]**
means measured, **[E]** estimated.

## Headline

- **It is fast.** Certifying the 5 packages that were attempted took 62.3 s
  [M], one after another, with warm caches and `make verify` loading the
  machine. With `contract check` (5.9 s) and one analysis (about 11.5 s), the
  whole loop is about 80 s [E].
- **It covers most of the imports.** Core imports 17 non-builtin packages [M]:
  - before: 11 were admitted from the compiled-in tier, utils was partly
    covered, 5 had no contract, and 2 are workspace links;
  - after: 13 of 17 are admitted (76 %) [M].
- **What users see.** Acceptance-gate import sites went from 131 to 85
  (-35 %) [M]; leaving out the test-only `@solidjs/testing-library`, 55 to 9.
  No violation moved, and no non-`SC9005` finding was gained. The status stays
  `violation`.

## Per package [M]

| package | wall | exit | outcome |
| --- | ---: | ---: | --- |
| `@solid-primitives/form@1.0.0-next.3` | 24.2 s | 0 | certified; environment has 7 entries (signals rc.3); `createFormResetListener`, the one export core uses, got no closure candidate |
| `@solid-primitives/interaction@1.0.0-next.4` | 10.1 s | 2 | **refused whole**: the probe worker died with `[REACTIVITY_HALTED] ReferenceError: document is not defined` in `ariaHideOutside` |
| `@solid-primitives/utils@7.0.0-next.4` (`.`, `./colors`) | 21.7 s | 0 | certified as a case set; `access` closes on all four domains |
| `@solidjs/testing-library@1.0.0-beta.2` | 3.5 s | 2 | refused at planning: `aria-query@5.3.0` has "no runtime ESM exports" |
| `vite-plugin-solid@3.0.0-next.5` | 2.8 s | 0 | "certified", but its environment was **not acquired** ("assert is not installed above …/@babel/helper-module-imports"), so it is admitted nowhere |

## packages/core, baseline -> with the catalog [M]

| id | baseline | with the catalog |
| --- | ---: | ---: |
| SC9011 | 20 | 5 |
| SC9005 | 119 | 146 |
| SC1001, SC2001, SC4001, SC5003, SC9012 | 223, 60, 15, 4, 169 | unchanged |

Each changed finding was classified by hand:

- **31 `access` acceptance-gate sites and 2 "callback execution" findings
  cleared.** `createMemo(() => … access(options))` in
  `i18n/create-date-formatter.ts:24` and `create-number-formatter.ts:28` is
  now known to call synchronously. This was false uncertainty, correctly
  removed.
- **15 `SC9011` became 15 `SC9005` open-claims findings** at the same
  line and column. Form's one collapsed acceptance-gate finding also became 31
  per-call warnings. This is the same gap, blamed more specifically: no
  regression and no removed false positive.
- **Net: +12 findings**, all `SC9005`. Honest, but noisier.

## Controls [M]

- **Without the trust configuration:** exit 2, "policy-2 acceptance receipt
  requires authenticated issuer provenance", and **no findings at all**.
  Nothing unauthenticated got in, but once a project has a catalog, every
  analysis run without the trust file loses all analysis.
- **The same catalog in a signals rc.6 tree:** form is not admitted, and
  `contract check` names why: "dependency environment differs:
  @solidjs/signals installed 2.0.0-rc.6, certified 2.0.0-rc.3". The findings
  were byte-identical with and without the catalog. A signals rc.3 tree under
  bun, instead of pnpm, does admit it, so the control is not vacuous.
- **Determinism:** 4 runs gave byte-identical output.

## Product defects this found

1. **A project catalog without its trust file breaks analysis entirely.** It
   should fall back to "catalog not authenticated", with the findings intact.
2. **One probe crash refuses a package's whole certification.** The crash was
   `document is not defined` under Node with no DOM. It should withhold the
   claims that crash, and keep the rest.
3. **Node built-ins count as missing dependencies** in environment acquisition
   (`assert`, required by babel). So `vite-plugin-solid` "certifies" with exit
   0 and is admitted nowhere.
4. **`packageSummaries` says `"evidence": "accepted"`** for an entry that was
   not admitted: vite-plugin-solid, and form in the rc.6 tree.
5. **A catalog is found only in its own project directory.** The monorepo root
   `tsconfig.json`, which analyses the same core files, sees no change.
6. **Certify leaves `.solid-checker-certification-*.mjs` files** in `.pnpm`
   (3 here).
7. **Open-claims `SC9005` is reported per call argument,** so admitting a
   partly closed contract increases the finding count.

## Re-run after defects 1-4 were fixed (65d64399)

Same tree and procedure, with a fresh issuer and release binaries at 65d64399.
Every number is measured.

| package | wall | exit | outcome, previous run in brackets |
| --- | ---: | ---: | --- |
| form | 22.5 s | 0 | certified [same] |
| interaction | 17.4 s | **0** | **publishes**; only `createHideOutside`/`returns` withheld ("the worker threw: ReferenceError: document is not defined") [refused whole, exit 2] |
| utils (`.`, `./colors`) | 22.0 s | 0 | certified [same] |
| testing-library | 2.7 s | 2 | refused, `aria-query` [same] |
| vite-plugin-solid | 1.8 s | 0 | environment acquired, 12 entries [0]; **not admitted** (below) |

- End to end: 84.1 s. Packages admitted: 14 of 18 non-builtin (78 %).
- `packages/core` acceptance-gate import sites: 131 (baseline) -> 85
  (previous run) -> **77**. Excluding testing-library: 55 -> 9 -> **1**.
- No violation moved, and no non-`SC9005` finding was gained. The 16 lost
  `SC9011` each became an `SC9005` at the same position.
- Defect 1: without trust, the JSON is byte-identical to no catalog, with one
  notice. Defect 2: see interaction. Defect 3: the environment is acquired.
  Defect 4: `packageSummaries` says `refused` with a detail. **All confirmed.**
- Controls: 4 runs byte-identical. In the signals rc.6 tree the catalog is
  refused with the named reason; the rc.3 twin admits it.

**New blocker: pnpm hoisting.** vite-plugin-solid is refused in the very tree
it was certified in: "merge-anything installed 6.0.6, certified 5.1.7".

- Its own `merge-anything` is 5.1.7. The 6.0.6 is pnpm's hoisted
  `.pnpm/node_modules` copy, which every other located package in the
  environment resolves.
- The admission rule requires every lookup from every located package to match,
  so it refuses. `solid-refresh` 0.7.8 against 0.8.0-next.7 hits the same
  thing.
- Certify exits 0 silently, because it never evaluates artifact admission.

Fix in progress: environment entries record which package resolved them;
admission checks each lookup from that importer; certify checks its own
admission and exits 1 if it fails.

Minor wording issues: the no-trust notice lists vite-plugin-solid as
admittable, and `packageSummaries` in the rc.6 tree says "no exact lockfile
integrity" for packages that are not installed there.
