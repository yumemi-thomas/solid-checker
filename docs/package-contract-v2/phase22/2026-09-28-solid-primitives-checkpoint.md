# The @solid-primitives checkpoint, scoped and measured

Measured 2026-09-28 at `061cc531` (after ADRs 0144, 0147 and 0148). Every
number below is **measured** unless it is marked *estimated*. The harness is
`scripts/primitives-checkpoint.mjs` and `make primitives-checkpoint`; the
corpus is pinned in `scripts/ecosystem-benchmark/primitives-checkpoint-corpus.json`
and the misuse ledger is `fixtures/primitives-misuse/cases.json`.

## The checkpoint

The owner's checkpoint (2026-09-28): **every `@solid-primitives/*` package with
a Solid 2 release has its contract generated and certified, and misuse issues
real warnings and errors.** The harness applies it as four criteria per
package:

1. its contract certifies from the published bytes host free, for `browser`
   and for `node` (ADR 0140), and every nameable entrypoint is in the accepted
   tier (`pkg/contracts/accepted/index.json`) for each of the three;
2. every export is certified clean in every host, or is uncertifiable only for
   a reason the owner has accepted as genuinely unprovable
   (`ACCEPTED_UNPROVABLE` in the script, **empty**: every cause the
   classifier names today is a checker gap);
3. every export with a misuse path has a ledger case that, against the real
   published typings, reports the expected rule on the misuse and nothing on
   the correct use, with `tsc` silent on both, in every host the case names;
4. the app-import metric shows the package's import sites certified. That
   metric is being built separately; the harness does not read it yet, and
   this report counts criterion 4 as **not measured**.

## Headline

| | value |
| --- | ---: |
| `@solid-primitives` packages in the org / with a Solid 2 release | 117 / **97** |
| packages at criteria 1-3 (criterion 4 not measured) | **1 of 97** (`platform`) |
| criterion 1: certified in all three hosts and in the tier for all three | 27 of 97 |
| criterion 2: every export clean in every host | 1 of 97 |
| criterion 3: every misuse path has a reporting case | 4 of 97 (all vacuous: no export has a misuse path, or no surface) |
| exports (union of the three hosts' surfaces) | 721 |
| exports accounted for (clean in all three hosts) | **93 of 721** (12.9 %) |
| exports clean: host free / `browser` / `node` | 96 / 96 / 93 |
| exports with a misuse path | 435, of which **203 stated by a contract claim** (in 70 packages) and 232 types-only hints |
| misuse fixtures: absent / present, not reporting / reporting | 434 / 1 / 0 |
| published-package defects (cannot load under rc.9) | 5 packages, plus 2 with an undeclared dependency |
| harness wall (release binary, 97 probes per host) | host free 268 s, `browser` 349 s, `node` 60 s; a second run through `make primitives-checkpoint` took 247 s / 289 s / 46 s, and 618 s end to end (release build check, three measurements, misuse ledger, report), under a load average of about 28 from concurrent work |

- **Only `platform` is at the checkpoint**, and only because its surface is 23
  constants (`isServer`, `isChrome`, ...): nothing callable, nothing to misuse.
- **Certification mostly holds already; the tier does not.** 91 of 97
  packages certify in all three hosts; the six that do not are the five
  published-package defects and `form`, refused under `node` (below). What
  fails criterion 1 is the tier: 68 packages have no bundle at all, and two
  (`form`, `storage`) have none for `node`, because the tier is regenerated
  from the census corpus (13 primitives packages) and the consumer
  environments, not from this corpus.
- **Criterion 2 is the real distance.** 628 of 721 exports are open in at
  least one host. The walls are the certification metric's walls, and no
  single one clears many exports alone (the greedy curves below).
- **Criterion 3 is blocked behind criterion 2.** The one ledger case
  (`timer` `createTimer` at module scope) shows the shape: the misuse
  reports `missing-owner` as a violation in every host, and `tsc` is silent on
  both files, but both the misuse and the correct use also report
  `package-contract-incomplete` (SC9005), because `createTimer`'s summary
  leaves callbacks, creates and reads open. No misuse fixture can pass while
  its export is not clean.
- **`node` is a different measurement.** Of the 63 packages the host-free
  run certifies on the published-graph lane, 60 fall back to the plain lane
  under `node` and `form` is refused, all because the graph refuses at
  `solid-js@2.0.0-rc.9 . [import,node]` (below); `storage` keeps the graph
  lane, and `sse` lands on the plain lane with no graph refusal recorded. The fallen-back exports are then held behind
  `@solid-primitives/utils` and the other `@solid-primitives` dependencies.
  That is also why the `node` run takes 60 s: the graph lane refuses before
  any dependency is certified.

## The package list

`bun scripts/primitives-checkpoint.mjs --select` pins the corpus from the
registry's authoritative org listing (`/-/org/solid-primitives/package`, 117
packages). A published version is a *Solid 2 release* when its `solid-js`
range (runtime dependency before peer, the benchmark's `solidRanges`
precedence) admits a published `solid-js` 2.x, or it depends on
`@solidjs/signals`; the newest such version is pinned, with its integrity. The
pin refuses a manifest row that disagrees with the registry, and
`--print-probes` refuses a manifest that no longer agrees with the pin. Each
package is measured by its manifest row's `head` probe (solid-js,
`@solidjs/web` and `@solidjs/signals` 2.0.0-rc.9, the audited triple).

Findings of the enumeration:

- **97 packages have a Solid 2 release**, and every one is already a `solid2`
  row of `scripts/ecosystem-benchmark/manifest.json` (generated 2026-09-27) at
  exactly the registry's newest Solid 2 version and integrity. 24 are in the
  certification-metric corpus and 13 in the census corpus.
- **The `next` dist-tag is not the release to pin.** For 17 packages `next`
  points at an older prerelease than `latest` (`a11y`: `next` 1.0.0-next.0,
  `latest` 1.0.0-next.3). The pin takes the newest Solid 2 version, which is
  also the manifest's.
- **The upstream `next` branch is one prerelease ahead of the registry for
  all 97** (`utils` 7.0.0-next.5 on the branch, 7.0.0-next.4 published): the
  pending changesets are unpublished, so they cannot be certified from
  published bytes yet. The branch has 102 package directories: the 97, plus
  `db-store`, `fetch`, `graphql`, `immutable` and `resource`, which still peer
  `solid-js` ^1.
- **20 packages have no Solid 2 release**: `autofocus`, `composites`,
  `cookies-store`, `countdown`, `date-difference`, `db-store`, `debounce`,
  `fetch`, `graphql`, `immutable`, `jsx-parser`, `local-store`,
  `page-visibility`, `reducer`, `resource`, `start`, `stream`, `throttle`,
  `until`, `visibility-observer` (the corpus file's `withoutSolid2`, with their
  dist-tags).

| package | pinned | `next` tag | metric | census | tier bundles none/browser/node |
| --- | --- | --- | :-: | :-: | --- |
| a11y | 1.0.0-next.3 | 1.0.0-next.0 |  |  | 1/1/1 |
| active-element | 3.0.0-next.2 | = | x |  | - |
| analytics | 2.0.0-next.2 | = |  |  | - |
| animation | 1.0.0-next.1 | = |  |  | - |
| async | 0.0.101-next.3 | 0.0.101-next.0 |  |  | - |
| audio | 3.0.0-next.2 | = | x |  | - |
| bounds | 1.0.0-next.2 | = | x |  | - |
| broadcast-channel | 1.0.0-next.2 | = |  |  | - |
| clipboard | 2.0.0-next.17 | = |  |  | - |
| connectivity | 1.0.0-next.2 | = |  |  | - |
| context | 2.0.0-next.2 | = |  | x | 1/1/1 |
| controlled-props | 1.0.0-next.3 | = |  |  | - |
| controlled-signal | 1.0.0-next.3 | 1.0.0-next.0 |  |  | 1/1/1 |
| cookies | 1.0.0-next.2 | = |  |  | - |
| cursor | 1.0.0-next.2 | = |  |  | - |
| date | 3.0.0-next.3 | = |  |  | - |
| deep | 1.0.0-next.3 | = |  |  | - |
| destructure | 1.0.0-next.2 | = |  |  | - |
| devices | 3.0.0-next.2 | = |  |  | - |
| drag-drop | 0.1.0-next.0 | = |  |  | - |
| event-bus | 3.0.0-next.3 | = | x |  | - |
| event-dispatcher | 1.0.0-next.2 | = |  |  | - |
| event-listener | 3.0.0-next.5 | = | x |  | 1/1/1 |
| event-props | 1.0.0-next.2 | = |  |  | - |
| favicon | 1.0.0-next.1 | 1.0.0-next.0 |  |  | - |
| filesystem | 3.0.0-next.3 | = |  |  | - |
| flux-store | 1.0.0-next.2 | = |  |  | - |
| focus | 1.0.0-next.4 | 1.0.0-next.0 |  |  | 1/1/1 |
| form | 1.0.0-next.3 | 1.0.0-next.0 |  |  | 1/1/0 |
| fullscreen | 2.0.0-next.3 | = |  |  | - |
| geolocation | 3.0.0-next.2 | = |  |  | - |
| gestures | 3.0.0-next.3 | = |  |  | - |
| history | 1.0.0-next.3 | = |  |  | - |
| i18n | 3.0.0-next.4 | = | x |  | 1/1/1 |
| idle | 1.0.0-next.3 | = |  |  | - |
| input-mask | 1.0.0-next.2 | = |  |  | - |
| interaction | 1.0.0-next.4 | 1.0.0-next.0 |  |  | 1/1/1 |
| intersection-observer | 3.0.0-next.3 | = |  |  | - |
| jsx-tokenizer | 3.0.0-next.2 | = |  |  | - |
| keyboard | 2.0.0-next.5 | = | x |  | 1/1/1 |
| keyed | 3.0.0-next.2 | = | x | x | - |
| lifecycle | 1.0.0-next.2 | = |  |  | - |
| list | 1.0.0-next.2 | = |  |  | - |
| list-state | 1.0.0-next.2 | 1.0.0-next.0 |  |  | - |
| map | 1.0.0-next.2 | = | x |  | 1/1/1 |
| marker | 2.0.0-next.2 | = |  | x | 1/1/1 |
| masonry | 2.0.0-next.2 | = |  |  | - |
| match | 1.0.0-next.2 | = |  |  | - |
| media | 4.0.0-next.2 | = | x |  | 1/1/1 |
| mediastream | 1.0.0-next.2 | 1.0.0-next.0 |  |  | - |
| memo | 2.0.0-next.2 | = | x | x | 1/1/1 |
| mouse | 4.0.0-next.3 | = |  |  | - |
| mutable | 3.0.0-next.2 | = |  |  | - |
| mutation-observer | 3.0.0-next.2 | = |  |  | - |
| notification | 1.0.0-next.3 | 1.0.0-next.0 |  |  | - |
| orientation | 1.0.0-next.2 | 1.0.0-next.0 |  |  | - |
| page-utilities | 3.0.0-next.2 | 3.0.0-next.0 |  |  | - |
| pagination | 1.0.0-next.8 | = |  |  | - |
| permission | 2.0.0-next.2 | = |  | x | 1/1/1 |
| platform | 1.0.0-next.2 | = |  | x | 1/1/1 |
| pointer | 1.0.0-next.2 | = |  |  | - |
| presence | 1.0.0-next.2 | = |  |  | 1/1/1 |
| promise | 2.0.0-next.2 | = |  |  | - |
| props | 4.0.0-next.3 | = | x |  | 1/1/1 |
| queue | 1.0.0-next.3 | 1.0.0-next.0 |  |  | - |
| raf | 4.0.0-next.2 | = |  |  | - |
| range | 1.0.0-next.3 | = |  |  | - |
| refs | 3.0.0-next.2 | = | x |  | 1/1/1 |
| resize-observer | 4.0.0-next.3 | = | x |  | 1/1/1 |
| rootless | 2.0.0-next.2 | = | x | x | 1/1/1 |
| scheduled | 2.0.0-next.2 | = | x | x | 1/1/1 |
| script-loader | 3.0.0-next.2 | = |  |  | - |
| scroll | 3.0.0-next.4 | = | x |  | 1/1/1 |
| selection | 1.0.0-next.2 | = |  |  | - |
| sensors | 1.0.0-next.3 | 1.0.0-next.0 |  |  | - |
| set | 1.0.0-next.2 | = |  |  | - |
| share | 4.0.0-next.4 | = |  |  | - |
| signal-builders | 1.0.0-next.4 | = |  |  | - |
| sortable | 1.0.0-next.0 | = |  |  | - |
| spring | 1.0.0-next.3 | = |  |  | - |
| sse | 1.0.0-next.2 | = |  |  | - |
| state-machine | 1.0.0-next.2 | = |  |  | - |
| static-store | 1.0.0-next.2 | = | x |  | 1/1/1 |
| storage | 5.0.0-next.4 | = | x | x | 1/1/0 |
| styles | 1.0.0-next.2 | = |  |  | - |
| timer | 1.4.5-next.1 | = | x | x | 1/1/1 |
| transition-group | 2.0.0-next.2 | = | x |  | - |
| trigger | 3.0.0-next.2 | = | x | x | 1/1/1 |
| tween | 2.0.0-next.2 | = |  | x | 1/1/1 |
| upload | 1.0.0-next.4 | = |  |  | 1/1/1 |
| url | 0.2.0-next.2 | 0.2.0-next.0 |  |  | - |
| utils | 7.0.0-next.4 | = | x | x | 3/3/3 |
| vibrate | 1.0.0-next.2 | 1.0.0-next.0 |  |  | - |
| video | 1.0.0-next.3 | 1.0.0-next.0 |  |  | - |
| virtual | 1.0.0-next.4 | = |  |  | - |
| websocket | 2.0.0-next.3 | = | x |  | - |
| workers | 2.0.1-next.1 | = |  |  | - |

## Per-package checklist

Criterion 1 columns: certification outcome per host (`defect` is a
published-package defect, below) and tier presence. Criterion 2: exports
clean in all three hosts, and exports open per host. Criterion 3: exports with
a misuse path stated by a claim / types-only, and exports whose case reports
correctly. The lane column is the host-free lane and the `node` lane.

| package | 1 certified none/browser/node | 1 tier | exports | 2 clean everywhere | 2 open (none/browser/node) | 3 misuse paths (claim/types) | 3 fixtures reporting | lane none → node | at 1-3 |
| --- | --- | :-: | ---: | ---: | --- | --- | ---: | --- | :-: |
| a11y | yes/yes/yes | yes | 7 | 0 | 7/7/7 | 4/2 | 0 | graph → plain |  |
| active-element | yes/yes/yes | no | 3 | 0 | 3/3/3 | 0/3 | 0 | graph → plain |  |
| analytics | yes/yes/yes | no | 10 | 0 | 10/10/10 | 1/6 | 0 | graph → plain |  |
| animation | defect/defect/defect | no | 0 | 0 | 0/0/0 | 0/0 | 0 | - → - |  |
| async | yes/yes/yes | no | 6 | 0 | 6/6/6 | 2/4 | 0 | plain → plain |  |
| audio | yes/yes/yes | no | 3 | 0 | 3/3/3 | 1/1 | 0 | graph → plain |  |
| bounds | yes/yes/yes | no | 2 | 0 | 2/2/2 | 0/1 | 0 | graph → plain |  |
| broadcast-channel | yes/yes/yes | no | 2 | 0 | 2/2/2 | 2/0 | 0 | plain → plain |  |
| clipboard | yes/yes/yes | no | 9 | 0 | 9/9/9 | 2/1 | 0 | graph → plain |  |
| connectivity | yes/yes/yes | no | 6 | 0 | 6/6/6 | 1/5 | 0 | graph → plain |  |
| context | yes/yes/yes | yes | 4 | 0 | 4/4/4 | 0/4 | 0 | plain → plain |  |
| controlled-props | defect/defect/defect | no | 7 | 0 | 7/7/7 | 0/6 | 0 | plain → plain |  |
| controlled-signal | yes/yes/yes | yes | 5 | 0 | 5/5/5 | 1/4 | 0 | graph → plain |  |
| cookies | yes/yes/yes | no | 4 | 0 | 4/4/4 | 3/0 | 0 | plain → plain |  |
| cursor | yes/yes/yes | no | 6 | 0 | 6/6/6 | 4/1 | 0 | graph → plain |  |
| date | yes/yes/yes | no | 22 | 9 | 13/13/13 | 8/2 | 0 | graph → plain |  |
| deep | yes/yes/yes | no | 4 | 0 | 4/4/4 | 0/1 | 0 | graph → plain |  |
| destructure | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/1 | 0 | graph → plain |  |
| devices | yes/yes/yes | no | 4 | 0 | 4/4/4 | 4/0 | 0 | plain → plain |  |
| drag-drop | defect/defect/defect | no | 13 | 0 | 13/13/13 | 0/5 | 0 | plain → plain |  |
| event-bus | yes/yes/yes | no | 11 | 0 | 11/11/11 | 2/3 | 0 | graph → plain |  |
| event-dispatcher | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/0 | 0 | plain → plain |  |
| event-listener | yes/yes/yes | yes | 11 | 0 | 11/11/11 | 2/8 | 0 | graph → plain |  |
| event-props | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/0 | 0 | plain → plain |  |
| favicon | defect/defect/defect | no | 11 | 0 | 11/11/11 | 0/6 | 0 | plain → plain |  |
| filesystem | yes/yes/yes | no | 15 | 0 | 15/15/15 | 5/3 | 0 | plain → plain |  |
| flux-store | yes/yes/yes | no | 4 | 0 | 4/4/4 | 1/2 | 0 | plain → plain |  |
| focus | yes/yes/yes | yes | 8 | 0 | 8/8/8 | 2/2 | 0 | graph → plain |  |
| form | yes/yes/refused | partial | 7 | 0 | 7/7/7 | 1/0 | 0 | graph → plain (generated) |  |
| fullscreen | yes/yes/yes | no | 3 | 0 | 3/3/3 | 1/2 | 0 | graph → plain |  |
| geolocation | yes/yes/yes | no | 6 | 0 | 6/6/6 | 2/3 | 0 | graph → plain |  |
| gestures | yes/yes/yes | no | 11 | 2 | 9/9/9 | 9/0 | 0 | plain → plain |  |
| history | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/1 | 0 | graph → plain |  |
| i18n | yes/yes/yes | yes | 12 | 0 | 12/12/12 | 3/4 | 0 | plain → plain |  |
| idle | yes/yes/yes | no | 1 | 0 | 1/1/1 | 1/0 | 0 | graph → plain |  |
| input-mask | yes/yes/yes | no | 7 | 1 | 6/6/6 | 0/4 | 0 | plain → plain |  |
| interaction | yes/yes/yes | yes | 5 | 0 | 5/5/5 | 3/0 | 0 | graph → plain |  |
| intersection-observer | yes/yes/yes | no | 11 | 3 | 8/8/8 | 4/2 | 0 | graph → plain |  |
| jsx-tokenizer | yes/yes/yes | no | 4 | 0 | 4/4/4 | 2/1 | 0 | graph → plain |  |
| keyboard | yes/yes/yes | yes | 7 | 0 | 7/7/7 | 0/7 | 0 | graph → plain |  |
| keyed | yes/yes/yes | no | 6 | 0 | 6/6/6 | 6/0 | 0 | plain → plain |  |
| lifecycle | yes/yes/yes | no | 3 | 0 | 3/3/3 | 2/0 | 0 | plain → plain |  |
| list | yes/yes/yes | no | 2 | 0 | 2/2/2 | 1/1 | 0 | graph → plain |  |
| list-state | yes/yes/yes | no | 2 | 0 | 2/2/2 | 2/0 | 0 | graph → plain |  |
| map | yes/yes/yes | yes | 4 | 0 | 4/4/4 | 0/1 | 0 | graph → plain |  |
| marker | yes/yes/yes | yes | 2 | 0 | 2/2/2 | 2/0 | 0 | plain → plain |  |
| masonry | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/1 | 0 | graph → plain |  |
| match | yes/yes/yes | no | 3 | 0 | 3/3/3 | 0/3 | 0 | plain → plain |  |
| media | yes/yes/yes | yes | 6 | 0 | 6/6/6 | 1/3 | 0 | graph → plain |  |
| mediastream | yes/yes/yes | no | 5 | 0 | 5/5/5 | 4/0 | 0 | graph → plain |  |
| memo | yes/yes/yes | yes | 7 | 0 | 7/7/7 | 5/2 | 0 | graph → plain |  |
| mouse | yes/yes/yes | no | 8 | 0 | 8/8/8 | 1/4 | 0 | graph → plain |  |
| mutable | yes/yes/yes | no | 2 | 0 | 2/2/2 | 0/1 | 0 | plain → plain |  |
| mutation-observer | yes/yes/yes | no | 2 | 0 | 2/2/2 | 1/1 | 0 | graph → plain |  |
| notification | yes/yes/yes | no | 4 | 0 | 4/4/4 | 1/2 | 0 | graph → plain |  |
| orientation | yes/yes/yes | no | 2 | 0 | 2/2/2 | 1/1 | 0 | graph → plain |  |
| page-utilities | yes/yes/yes | no | 4 | 0 | 4/4/4 | 1/2 | 0 | graph → plain |  |
| pagination | yes/yes/yes | no | 4 | 1 | 3/3/3 | 1/2 | 0 | graph → plain |  |
| permission | yes/yes/yes | yes | 1 | 0 | 1/1/1 | 1/0 | 0 | plain → plain |  |
| platform | yes/yes/yes | yes | 23 | 23 | 0/0/0 | 0/0 | 0 | plain → plain | **yes** |
| pointer | yes/yes/yes | no | 7 | 0 | 7/7/7 | 1/4 | 0 | graph → plain |  |
| presence | yes/yes/yes | yes | 1 | 0 | 1/1/1 | 0/1 | 0 | graph → plain |  |
| promise | yes/yes/yes | no | 7 | 0 | 7/7/7 | 1/2 | 0 | graph → plain |  |
| props | yes/yes/yes | yes | 8 | 1 | 7/7/7 | 0/3 | 0 | graph → plain |  |
| queue | yes/yes/yes | no | 6 | 0 | 6/6/6 | 3/3 | 0 | graph → plain |  |
| raf | yes/yes/yes | no | 4 | 0 | 4/4/4 | 4/0 | 0 | graph → plain |  |
| range | yes/yes/yes | no | 7 | 0 | 7/7/7 | 6/1 | 0 | graph → plain |  |
| refs | yes/yes/yes | yes | 8 | 0 | 8/8/8 | 4/2 | 0 | graph → plain |  |
| resize-observer | yes/yes/yes | yes | 7 | 0 | 7/7/7 | 3/1 | 0 | graph → plain |  |
| rootless | yes/yes/yes | yes | 8 | 0 | 8/8/8 | 3/5 | 0 | graph → plain |  |
| scheduled | yes/yes/yes | yes | 6 | 0 | 6/6/6 | 1/5 | 0 | plain → plain |  |
| script-loader | yes/yes/yes | no | 1 | 0 | 1/1/1 | 1/0 | 0 | plain → plain |  |
| scroll | yes/yes/yes | yes | 6 | 0 | 6/6/6 | 1/1 | 0 | graph → plain |  |
| selection | yes/yes/yes | no | 2 | 0 | 2/2/2 | 1/0 | 0 | graph → plain |  |
| sensors | yes/yes/yes | no | 10 | 0 | 10/10/10 | 5/5 | 0 | plain → plain |  |
| set | yes/yes/yes | no | 9 | 0 | 9/9/9 | 4/1 | 0 | graph → plain |  |
| share | yes/yes/yes | no | 35 | 32 | 3/3/3 | 2/0 | 0 | plain → plain |  |
| signal-builders | yes/yes/yes | no | 39 | 0 | 39/39/39 | 3/36 | 0 | graph → plain |  |
| sortable | yes/yes/yes | no | 12 | 0 | 12/12/12 | 2/7 | 0 | graph → plain |  |
| spring | yes/yes/yes | no | 3 | 0 | 3/3/3 | 3/0 | 0 | graph → plain |  |
| sse | yes/yes/yes | no | 12 | 0 | 11/11/12 | 2/0 | 0 | graph → plain |  |
| state-machine | yes/yes/yes | no | 1 | 0 | 1/1/1 | 0/1 | 0 | graph → plain |  |
| static-store | yes/yes/yes | yes | 3 | 0 | 3/3/3 | 1/1 | 0 | graph → plain |  |
| storage | yes/yes/yes | partial | 11 | 0 | 9/9/11 | 0/1 | 0 | graph → graph |  |
| styles | yes/yes/yes | no | 4 | 0 | 4/4/4 | 0/2 | 0 | graph → plain |  |
| timer | yes/yes/yes | yes | 5 | 0 | 5/5/5 | 4/1 | 0 (1 present) | plain → plain |  |
| transition-group | yes/yes/yes | no | 2 | 0 | 2/2/2 | 2/0 | 0 | graph → plain |  |
| trigger | yes/yes/yes | yes | 3 | 0 | 3/3/3 | 1/0 | 0 | graph → plain |  |
| tween | yes/yes/yes | yes | 1 | 0 | 1/1/1 | 1/0 | 0 | plain → plain |  |
| upload | yes/yes/yes | yes | 6 | 0 | 6/6/6 | 0/3 | 0 | plain → plain |  |
| url | yes/yes/yes | no | 12 | 0 | 12/12/12 | 0/2 | 0 | graph → plain |  |
| utils | yes/yes/yes | yes | 99 | 21 | 78/78/78 | 30/18 | 0 | plain → plain |  |
| vibrate | yes/yes/yes | no | 6 | 0 | 6/6/6 | 2/0 | 0 | graph → plain |  |
| video | yes/yes/yes | no | 7 | 0 | 7/7/7 | 1/4 | 0 | graph → plain |  |
| virtual | defect/defect/defect | no | 2 | 0 | 2/2/2 | 0/2 | 0 | plain → plain |  |
| websocket | yes/yes/yes | no | 10 | 0 | 10/10/10 | 7/0 | 0 | plain → plain |  |
| workers | yes/yes/yes | no | 5 | 0 | 5/5/5 | 3/1 | 0 | plain → plain |  |

## What blocks the exports

Each open consumer domain of each non-clean export gets one cause from the
certification metric's classifier (`certification-metric.mjs` `causeOf`, read
strictly from the answering case's records), per host. The table joins the
three hosts by cause and counts distinct exports blocked in any host; the
host columns are per-host counts. One class is new here: **graph lane
refused** attributes an `unaccepted dependency` cause to the published-graph
lane's own preparation refusal when the row's graph lane refused, because the
dependency is unaccepted *because* the graph that would have accepted it
prepared nothing.

| # | class | wall | exports | none | browser | node | packages |
| ---: | --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | graph lane refused | graph node solid-js@2.0.0-rc.9 [import,node]: export "action" re-exported from @solidjs/signals, which no planned dependency binds | 352 | 0 | 0 | 352 | 60 |
| 2 | recipe | no probe recipe (reads) | 302 | 294 | 299 | 66 | 65 |
| 3 | unaccepted dependency | @solid-primitives/utils | 265 | 9 | 9 | 265 | 46 |
| 4 | withheld operation | operation census refused: recursive-value-shape | 222 | 212 | 209 | 111 | 56 |
| 5 | missing claim form | returns never proposed | 208 | 207 | 207 | 28 | 67 |
| 6 | missing claim form | callbacks never proposed | 195 | 193 | 193 | 44 | 69 |
| 7 | declined | unresolved-callee | 169 | 125 | 163 | 127 | 56 |
| 8 | no record | proposed, not certified | 123 | 123 | 123 | 37 | 56 |
| 9 | census refusal | uncensused invoking form: property-access-unknown-accessor | 119 | 94 | 119 | 45 | 49 |
| 10 | withheld operation | operation census refused: callable-path | 102 | 89 | 93 | 22 | 41 |
| 11 | dialect-silent | solid-js:createSignal | 85 | 83 | 2 | 83 | 50 |
| 12 | declined | runtime-accessor-installation | 83 | 83 | 83 | 30 | 18 |
| 13 | missing claim form | reads never proposed | 73 | 73 | 73 | 1 | 28 |
| 14 | dialect-silent | solid-js:createMemo | 68 | 68 | 0 | 68 | 17 |
| 15 | unaccepted dependency | @solid-primitives/event-listener | 65 | 0 | 0 | 65 | 11 |
| 16 | missing claim form | creates never proposed | 59 | 55 | 58 | 6 | 25 |
| 17 | dialect-silent | solid-js:createEffect | 40 | 11 | 38 | 12 | 24 |
| 18 | declined | create-publishing-callee | 33 | 30 | 31 | 0 | 16 |
| 19 | missing claim form | callbacks: invokes a caller-supplied callable | 33 | 31 | 33 | 19 | 17 |
| 20 | not certified | certification refused at artifact-case | 24 | 24 | 24 | 24 | 2 |
| 21 | recipe | veto did not complete | 21 | 9 | 9 | 17 | 10 |
| 22 | declined | refusing-callee-fixpoint | 18 | 16 | 16 | 16 | 11 |
| 23 | census refusal | no function-like declaration | 17 | 9 | 17 | 2 | 10 |
| 24 | attribution catch-all | dialect row scoped to the browser host (solid-js) | 16 | 16 | 0 | 8 | 11 |
| 25 | census refusal | uncensused invoking form: coercion | 15 | 12 | 15 | 9 | 9 |
| 26 | graph lane refused | @tauri-apps/api is not installed (imported by @solid-primitives/filesystem) | 15 | 15 | 15 | 15 | 1 |
| 27 | unaccepted dependency | fs/promises | 15 | 15 | 15 | 15 | 1 |
| 28 | census refusal | reads premise required: property-access-unknown-accessor | 13 | 13 | 13 | 11 | 5 |
| 29 | census refusal | unresolved callee | 13 | 10 | 13 | 8 | 4 |
| 30 | not certified | no accepted case for this entrypoint | 13 | 3 | 3 | 10 | 2 |

### Per host, by class

| class | host free: exports / solely | `browser` | `node` |
| --- | --- | --- | --- |
| missing claim form | 304 / 19 | 306 / 22 | 78 / 2 |
| recipe | 298 / 5 | 303 / 5 | 76 / 0 |
| withheld operation | 280 / 10 | 280 / 10 | 134 / 10 |
| declined | 230 / 6 | 265 / 6 | 166 / 3 |
| dialect-silent | 180 / 0 | 67 / 0 | 182 / 0 |
| census refusal | 170 / 3 | 214 / 3 | 82 / 2 |
| no record | 123 / 2 | 123 / 2 | 37 / 2 |
| not certified | 36 / 36 | 36 / 36 | 43 / 43 |
| attribution catch-all | 32 / 0 | 21 / 0 | 18 / 0 |
| unaccepted dependency | 31 / 3 | 31 / 4 | **384 / 147** |

Greedy combination (upper bound: the non-clean exports with no remaining cause
after adding each class):

| step | host free (of 625) | `browser` (of 625) | `node` (of 610) |
| ---: | --- | --- | --- |
| 1 | not certified 36 | not certified 36 | unaccepted dependency 147 |
| 2 | missing claim form 55 | missing claim form 58 | dialect-silent 295 |
| 3 | declined 115 | declined 126 | declined 375 |
| 4 | recipe 162 | recipe 175 | not certified 418 |
| 5 | withheld operation 237 | withheld operation 259 | withheld operation 440 |
| 6 | census refusal 358 | census refusal 424 | census refusal 472 |
| 7 | dialect-silent 450 | no record 514 | missing claim form 508 |
| 8 | no record 562 | dialect-silent 573 | recipe 555 |

### The walls named in the scoping

- **Per-host `node` blocker at `solid-js` rc.9 server `action`: 352 exports,
  60 packages, `node` only.** The graph lane's `solid-js@2.0.0-rc.9 . [import,node]`
  node refuses: *Declaration target for export "action" is re-exported from
  dependency "@solidjs/signals" (module dist/types/core/action.d.ts), which no
  planned dependency binds because the resolved package has none*. The whole
  graph prepares nothing, the row falls back to the plain lane, and every
  export that needed an accepted `@solid-primitives/utils` (265),
  `event-listener` (65), `trigger` (13), `queue` (7), `memo`, `permission` (4
  each) and others stays open. `form` is the one row with no plain-lane
  fallback, so it is refused outright under `node`.
- **Missing claim forms: up to 306 exports (`browser`)** — `returns never
  proposed` 208, `callbacks never proposed` 195, `reads never proposed` 73,
  `creates never proposed` 59, and nested callbacks (`callbacks: invokes a
  caller-supplied callable`) 33 (distinct exports per wall, any host).
- **Recipes: 302 exports** have a reads claim withheld for want of a probe
  recipe, 21 more a veto that did not complete. A recipe may only uncover the
  census refusal underneath (the probe-recipe scaffold's two-pass procedure is
  the check); the recipe count is an upper bound on what recipes clear.
- **Census refusals: up to 214 exports (`browser`)** — chiefly `property-access-unknown-accessor`
  (119 as an invoking form, 13 as a reads premise), `coercion` 15, `no
  function-like declaration` 17, `unresolved callee` 13.
- **Withheld operations: 280 exports (host free, `browser`)** — `recursive-value-shape` 222,
  `callable-path` 102, `operation-reachability` 7, `operation-cardinality` 6.
- **Declines: up to 265 exports (`browser`)** — `unresolved-callee` 169, `runtime-accessor-installation`
  83, `create-publishing-callee` 33, `refusing-callee-fixpoint` 18.
- **Dialect rows** — `solid-js` `createSignal` 85 exports (83 host free and
  `node`, 2 under `browser`, where ADR 0124's browser-scoped rows apply),
  `createMemo` 68 (0 under `browser`), `createEffect` 40, `createStore` 8,
  `mapArray` 6, `onCleanup` 4, and 16 exports whose `solid-js` row is scoped to
  the browser host.

### Published-package defects

These are facts about the published bytes under rc.9, not checker gaps, and
whether they leave the denominator is an owner decision:

| package | defect |
| --- | --- |
| `animation@1.0.0-next.1` | the tarball has no `dist/index.js`, the runtime target its `exports` names |
| `controlled-props@1.0.0-next.3`, `drag-drop@0.1.0-next.0`, `favicon@1.0.0-next.1`, `virtual@1.0.0-next.4` | import `solid-js/web`, which `solid-js@2.0.0-rc.9` does not export (its subpaths are `.`, `./attribution`, `./internal`, `./refresh`); `upload` depends on `drag-drop` and its graph refuses on the same import |
| `keyed@3.0.0-next.2`, `share@4.0.0-next.4` | import `@solid-primitives/utils` without declaring it (no `dependencies`), so the graph lane cannot install it |
| `filesystem@3.0.0-next.3` | its Tauri adapter imports `@tauri-apps/api`, which it does not declare (its only declared optional peer is `chokidar`), so the graph lane cannot install it; the same 15 exports also stop at the Node builtin `fs/promises`, which no contract speaks for |

## Misuse paths (criterion 3)

A misuse path is a way to use an export wrongly that a catalog rule is meant to
report. The harness derives it from the export's contract, strongest first:
the accepted summary, then the generated (proposed) summary, which carries
what the generator derived before certification weakened it:

| claim | misuse | rule | exports |
| --- | --- | --- | ---: |
| owner requirement (`requires`/`requiresCleanup: required`) | called outside an owner: module scope, a detached callback | SC4001 `missing-owner` | 84 |
| returned accessor, or a returned callable that reads | the result read at a component's top level | SC1001 `strict-read-untracked` | 82 |
| reads a parameter, synchronously, untracked | called at a component's top level with a reactive argument | SC1001 | 58 |
| invokes a callback synchronously, tracked | a signal write inside the callback | SC2001 `reactive-write-in-owned-scope` | 21 |
| invokes a callback synchronously, untracked | a reactive read inside the callback | SC1001 | 4 |

203 exports in 70 packages have at least one claim-stated path (some have
several). Where no host's summary states any, the published declaration is
read (TypeScript over the tarball's `.d.ts`): a signature returning an
accessor-shaped callable (147 exports), taking a callback (98), or taking an
accessor argument (67). A types-only path is a hint that names the rule a
claim would feed; it is not a claim, and the misuse table in the harness
output marks it `types`. A zero-argument callable is accessor-shaped by type
whether it reads or not (`utils` `createIdGenerator` returns `() => string`),
so these are candidates to review, not paths.

The claim-stated paths, by package:

- **a11y**: `createAnnounce` (owner → SC4001), `createFormControl` (returned accessor → SC1001), `createFormControlInput` (owner → SC4001), `createReducedMotion` (owner → SC4001)
- **analytics**: `useAnalytics` (returned accessor → SC1001)
- **async**: `createAbortable` (owner → SC4001), `createAggregated` (returned accessor → SC1001; tracked callback → SC2001)
- **audio**: `createAudio` (owner → SC4001; returned accessor → SC1001)
- **broadcast-channel**: `createBroadcastChannel` (returned accessor → SC1001), `makeBroadcastChannel` (owner → SC4001)
- **clipboard**: `copyToClipboard` (owner → SC4001), `createClipboard` (owner → SC4001; returned accessor → SC1001; tracked callback → SC2001)
- **connectivity**: `createNetworkInformation` (returned accessor → SC1001)
- **controlled-signal**: `createControllableSignal` (returned accessor → SC1001)
- **cookies**: `createServerCookie` (owner → SC4001; returned accessor → SC1001), `createUserTheme` (returned accessor → SC1001), `parseCookie` (argument read → SC1001)
- **cursor**: `createBodyCursor` (owner → SC4001; tracked callback → SC2001), `createDragCursor` (owner → SC4001), `createElementCursor` (owner → SC4001), `makeElementCursor` (argument read → SC1001)
- **date**: `createCountdown` (owner → SC4001), `createDate` (returned accessor → SC1001), `createDateNow` (owner → SC4001; returned accessor → SC1001; tracked callback → SC2001), `createTimeAgo` (returned accessor → SC1001), `createTimeDifference` (returned accessor → SC1001), `formatDate` (argument read → SC1001), `getDateDifference` (argument read → SC1001), `getTime` (argument read → SC1001)
- **devices**: `createCameras` (returned accessor → SC1001), `createDevices` (owner → SC4001; returned accessor → SC1001 (proposed)), `createMicrophones` (returned accessor → SC1001), `createSpeakers` (returned accessor → SC1001)
- **event-bus**: `createEventStack` (returned accessor → SC1001), `toEffect` (owner → SC4001)
- **event-listener**: `createEventListener` (owner → SC4001), `eventListener` (owner → SC4001; tracked callback → SC2001)
- **filesystem**: `createFileSystem` (argument read → SC1001), `getItemName` (argument read → SC1001), `getParentDir` (argument read → SC1001), `makeVirtualFileSystem` (argument read → SC1001), `rsync` (argument read → SC1001)
- **flux-store**: `createFluxStore` (argument read → SC1001)
- **focus**: `createAutofocus` (owner → SC4001; tracked callback → SC2001), `createFocusGroup` (owner → SC4001)
- **form**: `createFormControlInput` (owner → SC4001)
- **fullscreen**: `fullscreen` (owner → SC4001)
- **geolocation**: `createGeolocation` (returned accessor → SC1001), `createGeolocationWatcher` (owner → SC4001; returned accessor → SC1001)
- **gestures**: `doubleTap` (owner → SC4001), `getCenterOfTwoPoints` (argument read → SC1001), `longPress` (owner → SC4001), `pan` (owner → SC4001), `pinch` (owner → SC4001), `registerPointerListener` (argument read → SC1001), `rotate` (owner → SC4001), `swipe` (owner → SC4001), `tap` (owner → SC4001)
- **i18n**: `proxyTranslator` (argument read → SC1001), `resolveRichTemplate` (argument read → SC1001), `resolveTemplate` (argument read → SC1001)
- **idle**: `createIdleTimer` (owner → SC4001; returned accessor → SC1001)
- **interaction**: `createHideOutside` (owner → SC4001), `createInteractOutside` (owner → SC4001; tracked callback → SC2001), `interactOutside` (owner → SC4001)
- **intersection-observer**: `createIntersectionObserver` (owner → SC4001), `createViewportObserver` (owner → SC4001), `createVisibilityObserver` (owner → SC4001), `makeIntersectionObserver` (owner → SC4001)
- **jsx-tokenizer**: `isToken` (argument read → SC1001), `resolveTokens` (returned accessor → SC1001; tracked callback → SC2001)
- **keyed**: `Entries` (returned accessor → SC1001), `Key` (returned accessor → SC1001), `MapEntries` (returned accessor → SC1001), `Rerun` (returned accessor → SC1001), `SetValues` (returned accessor → SC1001), `keyArray` (owner → SC4001)
- **lifecycle**: `createIsMounted` (returned accessor → SC1001 (proposed)), `onElementConnect` (owner → SC4001)
- **list**: `List` (returned accessor → SC1001)
- **list-state**: `createListState` (returned accessor → SC1001), `createMultiSelectListState` (returned accessor → SC1001)
- **marker**: `createMarker` (owner → SC4001), `makeSearchRegex` (argument read → SC1001)
- **media**: `createPrefersDark` (returned accessor → SC1001)
- **mediastream**: `createAmplitudeFromStream` (owner → SC4001; returned accessor → SC1001), `createAmplitudeStream` (owner → SC4001), `createScreen` (owner → SC4001; returned accessor → SC1001), `createStream` (owner → SC4001; returned accessor → SC1001)
- **memo**: `createLatest` (argument read → SC1001; returned accessor → SC1001), `createLatestMany` (argument read → SC1001; returned accessor → SC1001), `createLazyMemo` (returned accessor → SC1001), `createPureReaction` (owner → SC4001; untracked callback → SC1001), `createReducer` (returned accessor → SC1001)
- **mouse**: `getPositionToElement` (argument read → SC1001)
- **mutation-observer**: `createMutationObserver` (owner → SC4001)
- **notification**: `createNotification` (owner → SC4001; returned accessor → SC1001)
- **orientation**: `createOrientation` (owner → SC4001; returned accessor → SC1001)
- **page-utilities**: `createPageLeaveBlocker` (owner → SC4001; tracked callback → SC2001)
- **pagination**: `createSegment` (returned accessor → SC1001; tracked callback → SC2001)
- **permission**: `createPermission` (owner → SC4001; returned accessor → SC1001 (proposed))
- **pointer**: `getPositionToElement` (argument read → SC1001)
- **promise**: `until` (tracked callback → SC2001)
- **queue**: `createConcurrentTaskQueue` (returned accessor → SC1001), `createQueue` (returned accessor → SC1001), `createTaskQueue` (returned accessor → SC1001)
- **raf**: `createMs` (owner → SC4001), `createRAF` (returned accessor → SC1001), `default` (returned accessor → SC1001), `targetFPS` (tracked callback → SC2001)
- **range**: `IndexRange` (returned accessor → SC1001), `Range` (returned accessor → SC1001), `Repeat` (returned accessor → SC1001), `indexRange` (owner → SC4001), `mapRange` (owner → SC4001), `repeat` (owner → SC4001; tracked callback → SC2001)
- **refs**: `Ref` (owner → SC4001), `Refs` (owner → SC4001), `resolveElements` (tracked callback → SC2001), `resolveFirst` (tracked callback → SC2001)
- **resize-observer**: `createResizeObserver` (owner → SC4001), `getElementSize` (argument read → SC1001), `makeResizeObserver` (owner → SC4001)
- **rootless**: `createBranch` (untracked callback → SC1001), `createDisposable` (untracked callback → SC1001), `createSubRoot` (untracked callback → SC1001)
- **scheduled**: `createScheduled` (returned accessor → SC1001 (proposed))
- **script-loader**: `createScriptLoader` (owner → SC4001)
- **scroll**: `createPreventScroll` (owner → SC4001)
- **selection**: `createSelection` (owner → SC4001; returned accessor → SC1001)
- **sensors**: `createAccelerometer` (owner → SC4001; returned accessor → SC1001 (proposed)), `createBattery` (owner → SC4001; returned accessor → SC1001 (proposed)), `createCompass` (owner → SC4001; returned accessor → SC1001), `createGyroscope` (owner → SC4001; returned accessor → SC1001), `createSensor` (owner → SC4001; returned accessor → SC1001 (proposed))
- **set**: `difference` (returned accessor → SC1001), `intersection` (returned accessor → SC1001), `symmetricDifference` (returned accessor → SC1001), `union` (returned accessor → SC1001)
- **share**: `createSocialShare` (returned accessor → SC1001), `createWebShare` (returned accessor → SC1001)
- **signal-builders**: `capitalize` (tracked callback → SC2001), `lowercase` (tracked callback → SC2001), `uppercase` (tracked callback → SC2001)
- **sortable**: `insertSorted` (argument read → SC1001), `makeSorted` (argument read → SC1001)
- **spring**: `createDerivedSpring` (owner → SC4001), `createSpring` (owner → SC4001; returned accessor → SC1001), `makeSpring` (returned accessor → SC1001)
- **sse**: `lines` (argument read → SC1001), `ndjson` (argument read → SC1001)
- **static-store**: `createDerivedStaticStore` (tracked callback → SC2001)
- **timer**: `createIntervalCounter` (returned accessor → SC1001; tracked callback → SC2001), `createPolled` (owner → SC4001; returned accessor → SC1001 (proposed)), `createTimeoutLoop` (owner → SC4001), `createTimer` (owner → SC4001)
- **transition-group**: `createListTransition` (returned accessor → SC1001), `createSwitchTransition` (owner → SC4001)
- **trigger**: `createTrigger` (returned accessor → SC1001)
- **tween**: `createTween` (owner → SC4001; returned accessor → SC1001 (proposed))
- **utils**: `accessArray` (argument read → SC1001), `arrayEquals` (argument read → SC1001), `contains` (argument read → SC1001), `createHydratableSignal` (returned accessor → SC1001), `createHydrateSignal` (returned accessor → SC1001), `createMicrotask` (owner → SC4001), `filterNonNullable` (argument read → SC1001), `handleDiffArray` (argument read → SC1001), `lines` (argument read → SC1001), `ndjson` (argument read → SC1001), `wrapSetter` (argument read → SC1001), `./colors colorToOKLCH` (argument read → SC1001), `./colors complement` (argument read → SC1001), `./colors detectColorFormat` (argument read → SC1001), `./colors lighten` (argument read → SC1001), `./colors mix` (argument read → SC1001), `./colors perceptualColorScale` (argument read → SC1001), `./colors saturate` (argument read → SC1001), `./immutable drop` (argument read → SC1001), `./immutable dropRight` (argument read → SC1001), `./immutable fill` (argument read → SC1001), `./immutable filter` (argument read → SC1001), `./immutable filterInstance` (argument read → SC1001), `./immutable filterOutInstance` (argument read → SC1001), `./immutable flatten` (argument read → SC1001), `./immutable map` (argument read → SC1001), `./immutable remove` (argument read → SC1001), `./immutable shallowArrayCopy` (argument read → SC1001), `./immutable slice` (argument read → SC1001), `./immutable sort` (argument read → SC1001)
- **vibrate**: `createPulse` (owner → SC4001; returned accessor → SC1001), `createVibrate` (owner → SC4001; returned accessor → SC1001)
- **video**: `createVideoFrameCallback` (owner → SC4001; returned accessor → SC1001)
- **websocket**: `createReconnectingWS` (owner → SC4001), `createWS` (owner → SC4001), `createWSData` (returned accessor → SC1001), `createWSMessage` (argument read → SC1001; owner → SC4001; returned accessor → SC1001 (proposed)), `createWSState` (argument read → SC1001; returned accessor → SC1001 (proposed)), `createWSStore` (argument read → SC1001; owner → SC4001), `makeHeartbeatWS` (argument read → SC1001)
- **workers**: `createReactiveWorker` (owner → SC4001; returned accessor → SC1001 (proposed)), `createWorker` (owner → SC4001), `createWorkerQuery` (returned accessor → SC1001; tracked callback → SC2001)

Types-only hints by package: a11y 2, active-element 3, analytics 6, async 4, audio 1, bounds 1, clipboard 1, connectivity 5, context 4, controlled-props 6, controlled-signal 4, cursor 1, date 2, deep 1, destructure 1, drag-drop 5, event-bus 3, event-listener 8, favicon 6, filesystem 3, flux-store 2, focus 2, fullscreen 2, geolocation 3, history 1, i18n 4, input-mask 4, intersection-observer 2, jsx-tokenizer 1, keyboard 7, list 1, map 1, masonry 1, match 3, media 3, memo 2, mouse 4, mutable 1, mutation-observer 1, notification 2, orientation 1, page-utilities 2, pagination 2, pointer 4, presence 1, promise 2, props 3, queue 3, range 1, refs 2, resize-observer 1, rootless 5, scheduled 5, scroll 1, sensors 5, set 1, signal-builders 36, sortable 7, state-machine 1, static-store 1, storage 1, styles 2, timer 1, upload 3, url 2, utils 18, video 4, virtual 2, workers 1.

### The misuse ledger

`fixtures/primitives-misuse/cases.json` holds one case today,
`timer-createTimer-module-scope`. `--misuse` installs the pinned package with
the head probe's runtime (solid-js, `@solidjs/web` and an overridden
`@solidjs/signals`, all 2.0.0-rc.9) in `rust/target/primitives-checkpoint/misuse/`,
compiles both files with the tsc oracle's compiler options, and runs the
release checker on each, host free and with `--runtime-target browser|node`:

| host | `tsc` | misuse | correct use | verdict |
| --- | --- | --- | --- | --- |
| none, `browser`, `node` | silent on both | `missing-owner` violation (×2), `package-contract-incomplete` | `package-contract-incomplete` | correct use not clean |

The owner claim is doing its job; the case fails only on SC9005, which clears
when `createTimer`'s callbacks, creates and reads domains close.

## Order of work

Ranked by what each step unlocks across the family. "Owner" marks a step that
needs an owner decision before it is started.

1. **The `node` graph refusal at `solid-js@2.0.0-rc.9` `action`** (352 exports,
   60 packages, `node` only). Until it clears, `node` measures a different
   lane from the other two hosts, and every `node` answer below is behind it.
   The `node` greedy curve's first step (`unaccepted dependency`, 147 exports
   solely blocked) is this wall. It is a binding between the server build's
   declarations (re-exported from `@solidjs/signals`) and a closure that
   resolves no `@solidjs/signals`; ADR 0027 keeps `solid-js` a dialect
   foundation, so **owner**: should a foundation package be a graph node at all
   under `node`, or does the declaration-to-runtime join (ADR 0137) need a
   foundation arm?
2. **Missing claim forms for returns and callbacks** (208 + 195 exports, each
   in ~68 packages; plus 33 nested callbacks). These are the widest walls that
   are not a host artifact and the first non-trivial step of both greedy
   curves.
3. **`recursive-value-shape` and `callable-path` operation censuses** (222 and
   102 exports). Together the largest withheld-operation walls.
4. **Recipes for reads** (302 exports), with the scaffold's two-pass procedure
   first so the census refusal under each withheld claim is known; expect part
   of this wall to move into the census-refusal rows rather than clear.
5. **`property-access-unknown-accessor`** (132 exports as invoking form and
   reads premise) and **`unresolved-callee` declines** (169).
6. **Host-free and `node` dialect rows for `createSignal` and `createMemo`**
   (85 and 68 exports). **Owner**: ADR 0124 scoped these rows to `browser`
   because the server bodies differ (`dist/server.js`); a `node` row needs its
   own review against those bodies, and a host-free consumer receives
   host-free cases only (ADR 0140), so the host-free gap stays unless the host
   free certification is given rows it can honestly apply.
7. **Regenerate the tier from this corpus** (criterion 1 for 70 packages: 68
   with no bundle, `form` and `storage` with none for `node`).
   **Owner**: the tier is regenerated from the census run and the consumer
   environments; adding the 97-package corpus to what `make accepted-bundles`
   delivers changes what is compiled into the checker, and the tier only
   admits a consumer whose tree reproduces the certified environment.
8. **Published-package defects** (5 packages that cannot load, 2 with an
   undeclared dependency, `filesystem`'s undeclared Tauri peer). **Owner**:
   leave them in the denominator (they can never pass), or drop them from the
   checkpoint until upstream publishes fixed bytes; either way they are
   upstream issues, not checker work.
9. **`ACCEPTED_UNPROVABLE`**. **Owner**: nothing is accepted as genuinely
   unprovable yet, so criterion 2 means *clean*. A candidate would have to be
   a cause no claim form can ever state; none of today's classes is one.
10. **Misuse fixtures** follow criterion 2 package by package: the 203
    claim-stated paths are the worklist, one ledger case per path class per
    export, written as soon as that export is clean.

## Reproducing

```sh
make primitives-checkpoint                       # release build, 3 × 97 probes, misuse ledger, report
bun scripts/primitives-checkpoint.mjs --select   # network + gh: re-pin the corpus
```

Outputs are in `rust/target/primitives-checkpoint/`: `run{,-browser,-node}.json`
(the benchmark runs), `measure{,-browser,-node}.json` (per-host measurement),
`misuse.json` and `checkpoint.{json,md}`. The retained benchmark trees are
removed after each host is measured (`PRIMITIVES_CHECKPOINT_KEEP=1` keeps
them). Two runs over the same binary (the first run by hand with the same flags, the second through `make primitives-checkpoint`) produced byte-identical `checkpoint.json` apart from run timings. For scale: the certification metric's 30 probes took 270-280 s host free at `f67a9a31`; 97 probes now take 247-268 s (ADRs 0144, 0147, 0148; different corpus, so not a like-for-like speedup).
