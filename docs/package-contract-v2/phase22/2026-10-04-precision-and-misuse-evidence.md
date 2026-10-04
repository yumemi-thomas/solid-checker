# Precision and package-misuse evidence (2026-10-04)

This round measures two questions. Does the checker report violations
without false positives on real Solid 2 code? Does it report wrong usage of
packages? It is measured on three populations that are independent of each
other, and each claim below is tied to an artifact under `rust/target/`.

## 1. Precision on the development set (48 projects, 38 applications)

`rust/target/defect-sweep/` holds the baseline sweep, the two triages and
`sweep.sh` / `compare.mjs`.

| Stage | Violations (excluding `prefer-*`) | False-positive rate |
| --- | ---: | ---: |
| Baseline | about 2,420 | 80-88 % (two triages) |
| After the execution-role, summary-path and owner/boundary fixes (`sweep-fix4`) | 366 | 21.9 % (80 / 366, re-triaged by reading every finding) |
| Final (`sweep-v3`) | 263 | all 80 re-triaged false positives gone |

The final state differs from the baseline as follows:

- `strict-read-untracked` 1,962 -> 187
- `reactive-write-in-owned-scope` 239 -> 2
- `uncalled-accessor` 55 -> 0
- `no-direct-mutation` 11 -> 0
- `async-outside-loading-boundary` 43 -> 0

The 37 findings that the re-triage judged true defects split as follows:

- **33 are still reported.** This includes all five frozen-UI strict reads
  (`button.tsx:36`, `RouteDetail.tsx:1555`, jibe-run tabs, app-game
  `example-3`, solid-groove `LoopInfo`), the two throwing writes, and the
  leaf-owner and missing-owner set.
- **Four are lost.** Each is a `ref` that reaches an intrinsic element through
  a component: `<Dynamic component="input" ref>` (app-game `example-4.tsx`
  :98 and :134) and wrapper components that forward `ref` (openbot
  `ServerSettingsModal.tsx:195`, probus `Starred.tsx:765`). A component's
  `ref` is a prop, so it is no longer assumed ownerless. Proving the forward
  needs call resolution in the role stages; that is open.

Some 32 current sites have no row in the re-triage. They are mostly
path-key differences for projects in multi-project repositories; they are not
claimed here.

## 2. Precision on a held-out set (81 projects, 40 repositories)

`rust/target/heldout/` and `rust/target/heldout-triage/`.

- **Sample.** Repositories were found by GitHub code search for
  `"@solidjs/web"` in `package.json`. Repositories already in the corpus were
  excluded, the sample was seeded (`srand(20261004)`), and each repository was
  installed from its lockfile with scripts disabled. Most install pre-rc
  betas.
- **Triage.** One reader went through all 165 violations, judging each against
  the installed release.

| | False positives | Rate |
| --- | ---: | ---: |
| First run | 20 / 165 | 12.1 % (rc installs 7.3 %, rc.9 or later 1 / 21) |
| After this round's fixes | 4 / 149 | 2.7 % |

All 43 true defects and all 101 benign-true findings are still reported, and
the fixes added no new violation. The four remaining false positives are one
open pattern in one repository: a component that returns a function child (3),
and a setter reached only from listeners registered in an effect (1).

The 43 true defects on unseen code:

- 10 `onCleanup` / `createEffect` calls in an effect apply (never disposed);
- 18 writes that throw in dev;
- 6 single-argument `createEffect` calls (throw MISSING_EFFECT_FN on
  rc.1/rc.2);
- frozen reads in deez-run and omarchy;
- a frozen destructured prop in niama.

## 3. Wrong package usage

### Runtime channel (`solid-checker feedback run`)

This channel runs Solid's own dev diagnostics, attributed to the authored call
site (`misuse-runtime-ledger.mjs`, `rust/target/misuse-runtime-v3.json`,
`misuse-extra-v3.json`). It needs no package contract.

| Ledger | Cases | Packages | Detected | Silent | Other |
| --- | ---: | ---: | ---: | ---: | --- |
| `@solid-primitives` (`fixtures/primitives-misuse/cases.json`) | 123 | 61 | 105 (85 %) | 13 | 1 unattributed by design (`access(count)`), 4 harness errors |
| router, TanStack Router, TanStack Query (`misuse-extra-cases.json`) | 5 | 3 | 4 | 1 | |

- **No correct twin draws a claim.** A strict read performed inside a package,
  when the authored site is the export's own call (`createPolled(...)`), is
  reported as package behaviour, not as misuse.
- **Why the 13 are silent.** These packages guard their own lifecycle calls,
  so Solid never warns.
- **Excluded package.** `@tanstack/solid-virtual@3.13.40` cannot load under
  Solid 2: it imports `solid-js/store`.

### Static channel

- **Precision.** Across the 123-case ledger no correct twin draws a violation;
  there were two before this round. Package-internal reads are now
  uncertifiable.
- **The runtime target matters.**
  - The accepted tier already proves positive owner requirements (`min: 1`)
    for many exports, but only for the **browser** host. Host-free, an
    `if (isServer) return;` guard makes the registration possible-but-not-sure,
    so it is uncertifiable.
  - `feedback run` executes a client-rendered development build in Chromium.
    It now gives its native analysis exactly that runtime: `--runtime-target
    browser --runtime-build development --rendering csr`.
- **Results under that runtime** (`rust/target/misuse-runtime-v4.json`):

  | Static verdict | Cases | Runtime verdict |
  | --- | ---: | --- |
  | Proven violation | 45 (37 %; host-free was 17) | 44 detected, 1 harness error |
  | Uncertifiable | 12 | |
  | No finding (contract domain open) | 66 | 13 of them runtime-silent too |

  The two channels never disagree on a correct twin.
- **What blocks the rest.** A read-only pass
  (`rust/target/owner-claims-research/report.md`) traced it:
  - `missing-owner` (17 with no finding):
    - 7 ledger expectations are wrong. Those exports reach only
      `makeEventListener`'s `tryOnCleanup`, which tolerates no owner, and the
      runtime agrees: it is silent on them.
    - 5 lose a proven claim: an import obligation marks the export's owner
      requirements `Open`, or the runtime-alias merge rebuilds the export
      without them (`main.rs` `mark_summary_claims_unknown`,
      `unify_runtime_alias_summaries`). ADR 0174 keeps positive items through
      both. Four of the five now draw a static violation; see
      `2026-10-04-open-owner-requirements.md`.
    - 3 need ADR 0173's same-role cover.
    - The rest are argument- or capability-conditional, or a cleanup/effect
      disjunction.
  - `strict-read-untracked` (41 with no finding): the `returns` and `reads`
    census walls.

    | Cause | Cases |
    | --- | ---: |
    | `returns` never proposed | 10 |
    | `recursive-value-shape` | 9 |
    | `callable-path` | 8 |
    | `domain-exhaustiveness` | 6 |
    | `reads` premise refusals | several |

## What this does not show

- The held-out triage is one reader's judgement on code nobody ran.
  Each TRUE verdict rests on source plus the installed dist, and the
  development-set triage likewise.
- Uncertifiable counts were not triaged.
- The runtime channel only sees executed paths.
- Static package-misuse detection reaches 37 % under the browser runtime and
  stays limited for strict reads until the `returns`/`reads` census walls
  move.
