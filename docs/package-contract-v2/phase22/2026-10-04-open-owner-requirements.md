# Open owner-requirement lists keep their guaranteed items (2026-10-04)

ADR 0174 makes two generation paths keep a proven `min: 1` owner requirement:
an obligation that opens the export's list, and the runtime-alias merge. This
report measures the change on the browser tier, the runtime misuse ledger, and
the 48-project development sweep.

## How it was measured

- **Binaries.**
  - The baseline is this change's code built against the tier as checked in
    at `0ca1d38bb`. Generation is the only code path the change touches, so
    for a consumer this binary is HEAD.
  - The candidate is the same code with the regenerated tier compiled in.
  - Both are release builds.
- **Tier.** The `@solid-primitives` checkpoint corpus was re-certified under
  the browser host. On the first attempt 80 of 97 rows were refused: the probe
  Node was not the one the build pins, because `SOLID_CHECKER_PROBE_NODE` is
  only exported inside `make`. With it set, 92 rows certify. Of the five that
  do not:
  - four packages import `solid-js/web`, which `solid-js@2.0.0-rc.9` does not
    export;
  - one is `animation`, whose row has no certification attempt.

  `bundle-accepted-contracts.mjs --carry` then replaced the 201 browser
  bundles whose exact key the run re-certified and carried the other 1,247
  byte for byte. Every replaced key existed before, and no bundle was
  withdrawn. The run artifacts are under `rust/target/primitives-checkpoint/`.
- **Inputs restored.** `rust/target` had been removed, so the inputs were
  rebuilt from their pins:
  - the ledger's tooling app (`helge-dev`) and the 38 corpus apps through
    `app-import-metric.mjs --run`;
  - the case installs through `primitives-checkpoint.mjs --misuse`.

## What the tier now says

41 of the 201 replaced bundles state different claims; the other 160 differ
only in bytes. Every changed claim is a new owner-requirement item, except in
`timer`:

| Package | Exports gaining an owner item |
| --- | --- |
| `active-element` | `focus` |
| `event-listener` (16 environments) | `makeEventListenerStack` (`onCleanup(execute)`, called directly) |
| `focus` | `createFocusRestore`, `createFocusTrap` (computation) |
| `form` | `createForm` |
| `fullscreen` | `createFullscreen` |
| `keyboard` | `createKeyDown` (computation) |
| `pagination` | `createInfiniteScroll` |
| `presence` | `createPresence` (computation) |
| `raf` | `createRAF`, `default` |
| `resize-observer` | `createElementSize` |
| `rootless` (12 environments) | `createRootPool` |
| `styles` | `createRemSize` |
| `upload` | `createFileUploader`, `fileUploader` |
| `video` | `createVideo` |

`timer` moved for another reason, a commit between the last tier and this
one:

- `makeTimer`'s callback is now `queued`, which is what `setTimeout` does, and
  its `callbacks` closure is dropped;
- `createTimer` gains its callback items.

Both changes move toward fewer claims or toward positive items, and neither
closes anything.

## Runtime misuse ledger

`misuse-runtime-ledger.mjs` over the 123 cases, Chromium 1234
(`rust/target/misuse-runtime-baseline.json`,
`rust/target/misuse-runtime-candidate.json`):

| | Baseline | Candidate |
| --- | ---: | ---: |
| Static violation | 45 | 49 |
| Static uncertifiable | 12 | 12 |
| No static finding | 66 | 62 |
| Runtime detected / silent / unattributed / harness error | 105 / 13 / 1 / 4 | 105 / 13 / 1 / 4 |
| Correct twins with a static violation | 0 | 0 |

The baseline reproduces the previously recorded numbers exactly. The four
cases that move are all `missing-owner` at module scope, and the runtime
reports each one (`NO_OWNER_CLEANUP`):

- `resize-observer` `createElementSize`
- `raf` `createRAF`
- `fullscreen` `createFullscreen`
- `rootless` `createRootPool`

The fifth expected case, `transition-group` `createSwitchTransition`, still
has no static finding:

- The generator now proposes its `min: 1` computation.
- The certifier withdraws it by name: "operation census refused:
  operation-reachability … owner requirement has no exact dialect primitive
  call". The body's one `createRenderEffect` is in the observed call list, so
  the witness fails on the strict reachability floor. The producer does not
  state that call `unconditional`, although the generator's host-folded syntax
  walk does.
- The withdrawal is the fail-closed outcome. Why the producer's lower bound
  differs is open.

The seven `makeEventListener` / `tryOnCleanup` cases gain nothing, as
intended: their registration is a conditional `onCleanup` inside
`tryOnCleanup`, so no guaranteed item reaches the caller.

## Development sweep

The reconstructed sweep (`rust/target/defect-sweep/sweep.mjs`,
`compare.mjs`) covers the 38 pinned apps, 49 projects, one-shot. `en-passant`
does not install (a `git+ssh` dependency) and is analysed uninstalled on both
sides, which leaves 48 installed projects.

| Runtime | Violations | Added | Removed | Uncertifiable |
| --- | ---: | ---: | ---: | ---: |
| Default (host-free) | 264 -> 264 | 0 | 0 | 10,079 -> 10,079 |
| Browser development CSR | 264 -> 264 | 0 | 0 | 10,085 -> 10,085 |

Nothing moved under either runtime. The sweep shows that no finding was
added. It does not show that any corpus app reaches a changed export at a
certified version: primitives are a small share of these apps' imports, and
mostly at older pins.

## What this does not show

- Only the browser host was regenerated. Host-free and node bundles carry
  the claims of the tier as checked in.
- The census and consumer-environment runs were not repeated, because their
  inputs went with `rust/target`. Their bundles are carried unchanged and
  still authenticate. A full regeneration would also pick up whatever other
  commits since `09e298996` moved, as `timer` did here.

## Second round: ADR 0173's owner-call cover promoted

The cover proof moved from its experiment branch into the main verifier. The
generator now proposes `min: 1` where same-role alternative calls cover every
normal completion. The browser checkpoint was re-certified (92 of 97 again) and
bundled with `--carry`. 201 bundles were replaced; four exports gained a
guaranteed item, and none lost one:

| Package | Export | Item |
| --- | --- | --- |
| `event-listener` | `createEventListener` | computation, `min: 1` (in the checkpoint's environment) |
| `transition-group` | `createSwitchTransition` | computation, `min: 1` |
| `scroll` | `createScrollPosition` | computation, `min: 1` |
| `idle` | `createIdleTimer` | cleanup, `min: 1` |

The census withheld no owner-requirement operation in this run. The previous
run withheld two, `createSwitchTransition` and `createIdleTimer`, both on the
strict floor; the cover now proves both.

| Ledger | Round 1 | Round 2 |
| --- | ---: | ---: |
| Static violation | 49 | 53 |
| Static uncertifiable | 12 | 10 |
| No static finding | 62 | 60 |
| Correct twins with a static violation | 0 | 0 |

New violations, each runtime-detected:

- `createEventListener` at module scope and in an effect apply;
- `createScrollPosition`;
- `createSwitchTransition`.

## Third round: possibly-computed signal accessors (ADR 0175)

The census describes a `createSignal` accessor whose first argument grammar
cannot classify as `owned-memo`, and `solid-js@2.0.0-rc.9`'s own factories are
audited for that row. The browser checkpoint was re-certified (92 of 97) and
bundled with `--carry`: 39 exports gain an `owned-memo` `returns` claim, and
none loses one.

| Ledger | Round 2 | Round 3 |
| --- | ---: | ---: |
| Static violation | 53 | 60 |
| Static uncertifiable | 10 | 10 |
| No static finding | 60 | 53 |
| Correct twins with a static violation | 0 | 0 |

New violations, each `STRICT_READ_UNTRACKED` at runtime:

- `createTween`, `createPointerPosition`, `createWSState`, `createReducedMotion`;
- `resolveFirst`;
- `signal-builders` `capitalize` and `ceil`.

