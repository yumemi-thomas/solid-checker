# The first Solid 2 consumer measurement

Measured 2026-09-17, after the contract tier was regenerated from the Solid 2
census run. This is the first time the checker has been run over real Solid 2
consumer code with Solid 2 contracts compiled in, and it answers two questions
the frozen Solid 1.x-era measurements could not.

## The corpus, and the branch trap

The consumer corpus must be pinned **per repository by branch**, and for the
largest one the obvious branch name is wrong.

| repository | Solid 2 branch | solid-js |
| --- | --- | --- |
| `solidjs-community/solid-primitives` | `next` | 2.0.0-rc.0 |
| `kobaltedev/kobalte` | `solid2` | 2.0.0-rc.3 |
| `corvudev/corvu` | `features/solid-v2` | 2.0.0-experimental.1 |
| `solidjs/solid-docs` | none single | several `docs/solid-2-*` |

The primitives repository also has a branch named `solid2`. It is **not** the
Solid 2 line: all 85 of its packages still pin `solid-js@^1.9.7` and its last
commit is 2026-03-01. A sweep run against it would look like it worked and
would silently produce a 1.x denominator under a Solid 2 name. `next` is the
Solid 2 line, and it carries its version in `pnpm-workspace.yaml` under
`catalog:` rather than in any package.json.

`solid-docs` has no single Solid 2 branch and is excluded here rather than
guessed at.

## What the shipped tier changed, measured A/B

`kobalte`'s `packages/core`, analysed twice with the same binary, once with the
compiled-in contracts and once with `--no-bundled-contracts`:

| | with the tier | without |
| --- | --- | --- |
| findings | 649 | 555 |
| SC9005 | 139 | 45 |
| at the acceptance gate | 13 | 16 |
| naming open claim domains | **97** | **0** |
| SC1001 / SC9012 / SC2001 / SC9011 / SC4001 | 234 / 169 / 64 / 24 / 15 | identical |

**This is the transition the 2026-09-14 recensus recorded as never having
happened** — 2,585 of 2,585 findings at the acceptance gate, zero at the
open-claims gate. Ninety-seven findings now name which claim domains are open
for the export the consumer actually called, instead of saying no contract was
accepted.

**And it is the ceiling, in the same table.** Every proven-defect rule is
unchanged to the finding. Reading a contract currently buys a better-targeted
obligation and not one new violation, which is what
`docs/precision-backlog.md`'s 2026-09-17 entry is about: the contract format
cannot yet say that a package clears tracking, so no contract claim can promote
a read to a proven untracked one.

## Solid 2 demand, so far

Three of the four corpus roots. **Not** a replacement denominator — the pinned
census denominator stays frozen until `solid-docs` is resolved, because a
partial corpus shrinks it arbitrarily and every coverage percentage would rise
without a statement being proven.

| root | projects | pairs | sites | violation | certified | uncertifiable |
| --- | --- | --- | --- | --- | --- | --- |
| `solid-primitives@next` | 119 | 25 | 82 | 21 | 5 | 93 |
| `kobalte@solid2` | 5 | 9 | 9 | 2 | 2 | 1 |
| `corvu@features/solid-v2` | 20 | 2 | 19 | 9 | 2 | 9 |
| **total** | **144** | **36** | **110** | **32** | **9** | **103** |

Thirty-two projects report proven violations against real Solid 2 code, which
is the checker doing its actual job rather than raising obligations.

Two shape notes. `corvu`'s 19 sites are 18 of `esbuild-plugin-solid`, a build
tool the ecosystem corpus does not carry, plus one primitive — it consumes
almost no contracted package, and all 19 of its findings sit at the acceptance
gate. And `kobalte` is the only root where the open-claims gate is reached at
all, because it is the only one importing packages the shipped tier covers.

The Solid 2 demand is far smaller and differently shaped than the 1.x-era 159
pairs over 1,958 sites, for the reason the coverage baseline already records:
the Solid 2 releases of these packages dropped much of the API the 1.x-era
consumers called.

## The ranked worklist this produces

Every blocker the sweep named, on the demanded exports, in demand order. `noRecipe`
is the runtime-probe corpus gap; `census:` is the implementation census refusing
a form.

| sites | export | open | blocker |
| --- | --- | --- | --- |
| 10 | `@solid-primitives/utils` `access` | callbacks | census: call-time invocation |
| 9 | `@solid-primitives/rootless` `createHydratableSingletonRoot` | reads, creates | `noRecipe`, census |
| 2 | `@solid-primitives/rootless` `createSingletonRoot` | reads, creates | `noRecipe`, census |
| 2 | `@solid-primitives/trigger` `TriggerCache` | reads | `noRecipe` |
| 1 | `@solid-primitives/memo` `createLazyMemo` | reads, creates | `noRecipe`, census |
| 1 | `@solid-primitives/permission` `createPermission` | reads | `noRecipe` |
| 1 | `@solid-primitives/trigger` `createTriggerCache` | callbacks, reads, creates | `noRecipe`, census |

Seven `@solid-primitives/utils` exports are already **fully closed** —
`noop`, `INTERNAL_OPTIONS`, `EQUALS_FALSE_OPTIONS`, `asArray`, `accessWith`,
`isDev`, `asAccessor`, `isServer` — which is what a determined negative looks
like and is not a gap.

## What is not done

- `solid-docs`, and therefore any re-pin of the demand denominator.
- The probe-recipe corpus, which is what `noRecipe` above is counting: 1 of 325
  recipes still addresses a claim.
- The tracking-vocabulary gap that caps what any of this can report.

### Update, 2026-09-18: the `noRecipe` half is done, and it was half a wall

Forty-one recipes were written against the worklist above and the coverage
census's own degenerate sites. `createHydratableSingletonRoot`,
`createSingletonRoot`, `TriggerCache` and `createTriggerCache` all close;
`access` does not, because its blocker was `census: call-time invocation` and
never a recipe. Corpus addressing went 53 of 325 to 80 of 366 and the consumer
sites it costs fell from 343 to 24; the census's degenerate bucket went 270 to
186.

The `noRecipe` reason turned out not to be a worklist at all — a candidate
withheld for one is weakened out of the plan before its census runs, so the
refusal underneath stays masked. `docs/precision-backlog.md`'s 2026-09-18 entry
has the measurement and names the three things that actually hold the remaining
186 sites.
