# Independent case recovery: full measurement

Baseline: `2026-09-07-mjs-source-full.json`, 322 complete / 52 partial /
24 refused / 20 not advanced, 768 accepted artifact selections.

After: `2026-09-07-independent-cases-full.json`, finished 2026-09-07
20:56:11 JST, exit 0, 876.231 seconds, no probe timeouts. The catalog-bound
counts are **322 complete / 60 partial / 27 refused / 9 not advanced**.

There are **69 new artifact cases at 40 distinct per-row entrypoints**, making
837 accepted selections. All 768 previous selections across 374 certified rows
are preserved, including runtime/declaration hashes, resolution branches and
multiplicity. There are now 382 certified rows. No metric definition changed.

## Exact entrypoint changes

Each row below had an empty accepted entrypoint and artifact-case set before.
The [machine evidence](2026-09-07-independent-cases-measurement.json) contains
every exact after-selection, file hash, branch, importer, receipt and catalog
pointer; repeated conditional selections are counted individually there.

| Probe | After entrypoints | Cases | Transition |
| --- | --- | ---: | --- |
| @solid-devtools/debugger 0.28.1 Solid 1 | `.`, `./index`, `./setup`, `./types` | 4 | not advanced → partial |
| @solid-devtools/shared 0.20.0 Solid 1 | `./detect`, `./primitives`, `./utils` | 5 | not advanced → partial |
| @solid-devtools/ui 0.10.3 Solid 1 | `./animation`, `./theme` | 2 | refused → partial |
| @solidjs/diagnostics 2.0.0-rc.3 Solid 2 | `.`, `./browser`, `./playwright`, `./protocol` | 4 | not advanced → partial |
| @solidjs/router 2.0.0-next.18 Solid 2 | `./fs`, `./server` | 2 | refused → partial |
| @solidjs/web 2.0.0-rc.3 Solid 2 | `./frames`, `./frames/client`, `./frames/server`, `./server-functions`, `./server-functions/client`, `./server-functions/rich-args`, `./server-functions/server`, `./storage` | 22 | refused → partial |
| solid-devtools 0.34.5 Solid 1 | `./vite` | 1 | refused → partial |
| solid-js 1.9.14 Solid 1 | `.`, `./dist/server.js`, `./h`, `./h/dist/h.js`, `./h/jsx-dev-runtime`, `./h/jsx-runtime`, `./html`, `./html/dist/html.js`, `./store`, `./store/dist/dev.js`, `./store/dist/server.js`, `./store/dist/store.js`, `./universal`, `./universal/dist/dev.js`, `./universal/dist/universal.js`, `./web/storage` | 29 | refused → partial |

The runtime for Debugger's `./types` is `./dist/types.js`, not a declaration-only
asset. Shared's source-condition cases authenticate the published `.ts` runtime
source. Assets, wildcard exports and deliberately scoped probes keep their
previous denominator treatment. **No row became complete.**

Eight further rows moved from not advanced to refused: Kobalte Core 0.13.13,
Solidbase 0.6.13, Controlled Props and Virtual Solid 2 floor/head, and Vite Plugin
Solid 2 floor/head. These are newly measured refusals, not certification gains.

## Implementation and consumer evidence

[ADR 0059](../../adr/0059-independent-artifact-case-recovery.md) describes the
bounded selection. A full attempt precedes recovery; trials verify additions
together with selected cases; final publication redoes native certification.
An explicit destination-state check prevents subset recovery over an existing
destination after full-request failure. Trial receipts are never copied.

Subset proposals retain whole cases and the exact referenced summaries. The
native proposal-census equality check remains intact. Snapshot replay, package
integrity, exact importer/resolution/conditions, dependency receipts, trust and
semantic witnesses remain mandatory. Each new row passed ordinary consumer
receipt authentication and exact case selection. Diagnostic expected/refused
case coordinates are not authority for any proof.

## Validation

- Contract workflow tests: 81 passed, covering fresh subset/final transactions,
  explicit refusals, no empty success, existing-destination protection,
  non-proof/final failures, the 32-case bound, exact projection identity and
  duplicate rejection.
- Runner tests: 65 passed, including opt-in scheduling of partial proposals
  without a dependency frontier and unchanged default scheduling.
- Full `make verify`: exit 0, `TOTAL 66.99s`, no `FAILED during step` marker.
  Log: `/private/tmp/independent-cases-verify.log`.
- Full corpus and exact catalog comparison passed. The comparison follows
  published pointers and validates file digests; native ordinary verification
  remains responsible for receipt signatures and trust.
- No fixture snapshots, protocol or receipt formats changed. No commit or push.

## Remaining shortfall and next work

The case recovery ceiling is not the overall coverage ceiling. Missing cases
still expose real shape/callback obligations and artifact replay mismatches.
Kobalte Core also exceeds the bounded recovery case count. Solidbase and Vite
Plugin now expose declaration export-census mismatches (`Article` and
`solidPlugin` respectively), which merit exact binding investigation.

Floating UI's initialized `list.concat`, Until/Observer callback flow,
generic/index/tuple shapes, CommonJS exports, runtime-library policy and actual
missing/invalid published artifacts remain on the
[active implementation plan](2026-09-07-coverage-ceiling-plan.md).
`DEFAULT_WEB_PLUGINS` is initialized through `Object.freeze`; a literal-only
premise would not address it. No unmeasured ceiling or complete-row gain is claimed.
