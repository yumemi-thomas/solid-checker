# What the regenerated tier changes for real Solid 2 consumers

Measured 2026-09-26. The checker was run over the same three Solid 2 consumer
trees twice. The first run used the checker at `c7473c46`, the last commit
before ADR 0122–0124, with the 2026-09-18 tier: index version 1, 137 bundles,
no environments. The second used HEAD `c6ab8d71`, with the tier regenerated
under ADR 0123 environment admission: index version 2, 162 bundles, every
bundle stating a `dependencyEnvironment`.

Every number below is **measured** unless it is marked *estimated*. The
scratch root for this run is
`/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/1aa3e519-86c0-43fe-acd0-487816ac2c51/scratchpad/sweep/`,
written `$SW` below. Nothing there is checked in.

## Headline

**The regenerated tier applies to none of the 151 consumer projects.** The one
project the old tier reached, `kobalte/packages/core`, lost all of it:

- **97 open-claims `SC9005` findings became 3 acceptance-gate findings.**
- **15 import bindings that the old tier certified are now uncertifiable.**
- No import moved from uncertifiable to certified.
- No finding of any other kind appeared or disappeared.
- The other 150 projects produced byte-identical output under both binaries.

That is ADR 0123 working as designed. The old tier admitted bundles certified
against `solid-js@2.0.0-rc.0` / `@solidjs/signals@2.0.0-rc.0` into a tree that
installs `solid-js@2.0.0-rc.3` / `@solidjs/signals@2.0.0-rc.3`. The new tier
refuses that. It also refuses its own `solid-js` rc.3 environment, because that
one was certified with `@solidjs/signals@2.0.0-rc.6`, the version a fresh
install resolved at census time, while kobalte's lockfile pins rc.3.

## The corpus

Shallow clones in `$SW/consumers/`, installed with
`CI=true pnpm install --frozen-lockfile --ignore-scripts` using each repo's own
pnpm (auto-switched from `packageManager`). All three resolved **without
scripts**.

| repository | branch | commit | commit date | pnpm | projects |
| --- | --- | --- | --- | --- | --- |
| `solidjs-community/solid-primitives` | `next` | `134c5cac19cc5f53dd5a394ecb42252184e8706b` | 2026-09-19 | 11.9.0 | 122 |
| `kobaltedev/kobalte` | `solid2` | `e9d426d438b7c9ea0cc81bd1133831a20cd5fcae` | 2026-09-24 | 11.20.0 | 7 |
| `corvudev/corvu` | `features/solid-v2` | `ffacbff0c286506ca87ef5f345d2e17db4ca0a99` | 2025-02-14 | 10.0.0 | 22 |

A project is one `tsconfig.json`, found by the sweep script's
`projects_under`: 151 in total.

### Installed Solid versions

`require.resolve('solid-js/package.json')` was run from representative project
directories: `packages/{utils,rootless,memo,storage}`, kobalte
`packages/{core,utils}` and `apps/docs`, and corvu `packages/{corvu,dialog}`.

| repository | `solid-js` resolved | `@solidjs/signals` | `@solidjs/web` | other `solid-js` in the store |
| --- | --- | --- | --- | --- |
| solid-primitives | 2.0.0-rc.9 | 2.0.0-rc.9 | 2.0.0-rc.9 | 1.9.7, 1.9.14 |
| kobalte | 2.0.0-rc.3 | 2.0.0-rc.3 | 2.0.0-rc.3 | 1.9.14 |
| corvu | 2.0.0-experimental.1 | 0.1.0 | 2.0.0-experimental.0 | — |

Six primitives projects resolve a 1.x `solid-js` and are refused with `SC9013`
under both binaries: `db-store`, `fetch`, `graphql`, `immutable`, `resource`
and `site`.

### Workspace links against published tarballs

This comes from each `pnpm-lock.yaml`: `importers` for the edges, and
`packages.*.resolution.integrity` for integrity.

- **solid-primitives.** 123 importer edges to 20 tier-named packages are
  `link:` workspace links, with no tarball and no integrity. The workspace
  versions are one prerelease ahead of the tier's, for example `utils`
  7.0.0-next.5 against the tier's next.4. Its published tier-named packages
  are all 1.x-era versions pulled in by the docs site, such as `utils` 6.2.3
  and 6.4.1 and `@kobalte/core` 0.13.13.
- **kobalte.** `@kobalte/core` (2.0.0-alpha.2) and `@kobalte/utils` are
  workspace links, so the 118 `@kobalte/core@2.0.0-alpha.0` bundles can never
  apply here. The `@solid-primitives/*@…-next.*` packages are published
  tarballs with integrity.
- **corvu.** No workspace links to tier packages. Its published primitives are
  1.x versions (`memo` 1.4.0, `scheduled` 1.5.0, `storage` 4.3.1, `utils`
  6.3.0).

## The tier-applicability ceiling

Every installed store instance of a package that some bundle names was matched
on `(packageName, packageVersion, packageIntegrity)` against both indexes. For
the new index, ADR 0123 § Admission was then replicated: walk Node resolution
from the instance's real path, then from each package found, and require every
environment entry's name, version and integrity. Script: `$SW/ceiling.py`.
Output: `$SW/ceiling.txt` and `$SW/ceiling.json`.

| repository | instances | identity match, old index | identity match, new index | admitted by the new index (*replicated rule*) |
| --- | --- | --- | --- | --- |
| solid-primitives | 12 | 0 | 0 | 0 |
| kobalte | 11 | 5 | 5 | **0** |
| corvu | 4 | 0 | 0 | 0 |

Kobalte's five identity matches are `keyed@3.0.0-next.2`,
`platform@1.0.0-next.2`, `rootless@2.0.0-next.2`, `trigger@3.0.0-next.2` and
`utils@7.0.0-next.4`. Every new-index bundle for them is refused on the first
environment entry the walk reaches:

- the rc.0 bundles: installed `@solidjs/signals` 2.0.0-rc.3, bundle says rc.0;
- the rc.3 bundles: installed `@solidjs/signals` 2.0.0-rc.3, bundle says rc.6;
- two `utils` `.` bundles: `@solid-primitives/storage` 4.3.5 installed, the
  bundle says 5.0.0-next.4.

**So the ceiling is 0 admitted instances in the whole corpus**, against 5 by
identity alone. The admission check is a reimplementation, so the 0 is
*estimated* on its own. The sweep below measures it: no open-claims finding
anywhere, and output identical with and without the tier.

## Sweep: old against new

Both binaries ran
`docs/package-contract-v2/phase21/2026-09-12-consumer-demand-measurement.py`,
byte-identical in both trees. The runs used `--report` from each tree's own
`benchmarks/ecosystem/report.json`, which is also byte-identical, over
`solid-primitives kobalte corvu`, from `$SW/consumers`.

- **old**: `$SW/old/rust/target/debug/solid-checker-rust`, from
  `make build-checker-debug` in a `c7473c46` worktree. It used that tree's
  `bin/solid-typefacts`, with the same source digest `01d360ec…` as HEAD but
  its own bytes. Output: `$SW/demand-old/`, `$SW/demand-old.txt`.
- **new**: `rust/target/debug/solid-checker-rust` at HEAD, sha256 `e96f862b…`,
  unchanged across the run, with the main tree's `bin/solid-typefacts`.
  Output: `$SW/demand-new/`, `$SW/demand-new.txt`.

Both runs analyzed 151 projects. Status was identical: 36 violation, 106
uncertifiable, 9 certified.

### SC9005 by gate

| gate | old | new |
| --- | --- | --- |
| acceptance ("no receipt-accepted contract matches this exact import") | 305 | 308 |
| open claims (`unknown-contract-claims:*`) | **97** | **0** |
| — `callbacks` | 78 | 0 |
| — `returns,ownerRequirements` | 16 | 0 |
| — `reactiveReads,returns,ownerRequirements` | 2 | 0 |
| — `returns` | 1 | 0 |
| callback execution (not an import site) | 1,134 | 1,134 |
| **SC9005 total** | **1,536** | **1,442** |

By repository (old → new):

| repository | acceptance | open claims | callback execution |
| --- | --- | --- | --- |
| solid-primitives | 217 → 217 | 0 → 0 | 1,058 → 1,058 |
| kobalte | 66 → 69 | 97 → 0 | 76 → 76 |
| corvu | 22 → 22 | 0 → 0 | 0 → 0 |

All of the change is in `kobalte/packages/core`. The old binary reported 13
acceptance and 97 open-claims findings there. The new one reports 16
acceptance findings. The three new acceptance findings are
`@solid-primitives/utils` (31 import sites), `@solid-primitives/utils/colors`
(12) and `@solid-primitives/platform` (10).

### Sites per (package, export)

The script's "import sites" total fell from 1,949 to 1,868. **That drop is a
change of unit, not a certification.** An open-claims finding is one per call
site. An acceptance-gate finding is collapsed to one per project and package,
with a site per import binding. The net −81 decomposes exactly:

| module | export | old sites (gate) | new sites (gate) |
| --- | --- | --- | --- |
| `@solid-primitives/utils` | `access` | 244 acceptance + 109 open claims | 275 acceptance |
| `@solid-primitives/utils/colors` | `parseColor` | 7 acceptance + 8 open claims | 14 acceptance |
| `@solid-primitives/utils/colors` | `COLOR_INTL_TRANSLATIONS` | 5 acceptance | 10 acceptance |
| `@solid-primitives/platform` | `isMac` | 6 acceptance | 10 acceptance |
| `@solid-primitives/platform` | `isWebKit` | 4 acceptance | 6 acceptance |
| `@solid-primitives/platform` | `isAppleDevice` | 3 acceptance | 5 acceptance |
| `@solid-primitives/platform` | `isIOS` | 2 acceptance | 4 acceptance |
| `@solid-primitives/utils/colors` | 17 re-exported names (`colorScale`, `mix`, `darken`, …) | 1 open claims each | 0 |

Every other (module, export) pair is unchanged. Full tables are in
`$SW/compare.txt` and in the two `demand-*.txt` files.

### Moved import bindings

The two gates count sites differently, so the moves were compared per import
binding `(project, file, module, export)`. Script: `$SW/bindings.py`. Output:
`$SW/bindings.txt` and `$SW/binding-moves.json`. All 71 moved bindings are in
`kobalte/packages/core`.

| old → new | bindings | exports |
| --- | --- | --- |
| **certified (no finding) → acceptance gate** | **15** | `platform` `isMac` 4, `isAppleDevice` 2, `isIOS` 2, `isWebKit` 2; `utils/colors` `COLOR_INTL_TRANSLATIONS` 5 |
| open claims → acceptance gate | 38 | `utils` `access` 31 (21 `returns`, 10 `callbacks`, covering 109 call-site findings); `utils/colors` `parseColor` 7 |
| open claims → no finding | 18 | the 18 names `src/colors/index.tsx` re-exports from `@solid-primitives/utils/colors` |
| uncertifiable → certified | **0** | — |

The last-but-one row is **not a certification**. With no accepted contract,
the checker raises nothing at a re-export declaration. The obligation is
reported where the package is imported directly: 12 sites in 11 `color-*`
files. The old binary with `--no-bundled-contracts` does the same, byte for
byte (see Controls), so this is how the acceptance gate already behaved, not a
new gap. Whether a barrel re-export should carry its own acceptance-gate site
is an open question. It is not answered here.

### Findings of other kinds

| | old | new |
| --- | --- | --- |
| non-SC9005 findings that appeared | — | **0** |
| non-SC9005 findings that disappeared | **0** | — |

Per-id totals are identical: SC1001 890, SC1003 6, SC1007 38, SC2001 1,089,
SC2003 22, SC4001 860, SC5003 14, SC6001 8, SC7001 46, SC8015 64, SC9011 108,
SC9012 926, SC9013 6. Nothing new appeared, so no finding needed a
`tsc`-duplication check.

## Controls

Five of the seven kobalte projects (root, `.storybook`, `apps/docs`,
`packages/core`, `packages/utils`) were rerun with `--no-bundled-contracts`
under both binaries. Output: `$SW/control/`.

- For every project, old without the tier == new without the tier ==
  **the new sweep**, byte for byte. So the new tier contributes nothing to
  kobalte. The code between `c7473c46` and HEAD (ADR 0122's tracking words,
  ADR 0124's host-scoped rows, admission) moves no finding there either.
- For `packages/core`, old without the tier differs from the old sweep, and
  its SC9005 split (16 acceptance, 0 open claims) is exactly the new sweep's.
  The whole old-to-new difference is the old tier's admission.
- Every other project in the sweep: 150 of 151 result files are
  byte-identical between old and new.

This agrees with the 2026-09-17 measurement. That run found 97 open-claims
findings in `kobalte/packages/core` with the tier and 0 without. Those 97 are
exactly the findings ADR 0123 now withholds.

## Wall time

| run | wall | binary |
| --- | --- | --- |
| old sweep, 151 projects | **844 s** | debug, `c7473c46` |
| new sweep, 151 projects | **891 s** | debug, HEAD |
| `kobalte/packages/core` alone, new, before the sweeps | 70.8 s | debug |
| `make build-checker-debug`, HEAD | 0.3 s (already current) | |
| `make build-checker-debug`, old worktree | 37 s | |

The two sweeps ran **concurrently** on one machine: 14 cores, on AC power,
`powermode 0`, load about 5 at start. The absolute walls are therefore
inflated by each other. The +47 s (+5.6%) difference was measured under
symmetric load, but it is one sample each. Both walls come from
`$SW/{old,new}.{start,end}`.

## Caveats

- **Debug binaries.** Both sweeps used `make build-checker-debug` builds. The
  daemon path is on by default only in release (`daemon::enabled()`). Tier
  admission is shared across callers (`diagnostics::project_accepted_contracts`),
  but a release rerun was not done.
- **Corpus scope.** Three roots, and `solid-docs` is still excluded, as on
  2026-09-17. Only kobalte installs published next-line primitives at all.
  solid-primitives consumes its own packages as workspace links, and corvu is
  on `solid-js@2.0.0-experimental.1` with 1.x-era primitives. So on this
  corpus the tier's reach was one project before and none after. That says
  more about the corpus than about the tier.
- **The tier's environments are not this corpus's.** The new tier states two
  environments: `solid-js` rc.0 with signals rc.0, and `solid-js` rc.3 with
  signals rc.6. The consumers install rc.9 with signals rc.9, rc.3 with signals
  rc.3, and experimental.1 with signals 0.1.0. None of those matches.
  *Estimated*, not measured here: a consumer would need a lockfile that
  reproduces a census row's exact environment, down to transitive primitives
  such as `storage@5.0.0-next.4` for `utils` `.`.
- **The ceiling's admission column reimplements ADR 0123 in Python.** The
  sweep and the controls confirm its 0 for kobalte. It was not checked against
  the Rust admission instance by instance.
- **Binding attribution.** `$SW/bindings.py` names acceptance-gate bindings by
  reading the import declaration's bytes (`value_bindings`), and names
  open-claims bindings by the per-export message. Re-exports are not parsed on
  the acceptance side. That is why they show as "no finding" there, and it
  matches the checker's output.
- **The consumer branches move.** The commits above are the measured state.
  Kobalte's `solid2` was 2 days old and primitives' `next` 7 days old at
  measurement time.
