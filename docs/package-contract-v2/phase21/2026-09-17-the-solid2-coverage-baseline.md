# The Solid 2 coverage baseline

- Measured 2026-09-17, `make contract-coverage-census` at `--solid 2`
- Pinned as `benchmarks/ecosystem/coverage-census.json`, `solidTarget: "solid2"`
- The Solid 1.x baseline it replaces is **not** superseded and **not** a
  continuation. It is recorded here beside it, per ADR 0110 § 5.

## Both baselines

| | solid1 (frozen) | solid2 (new pin) |
| --- | --- | --- |
| measured call sites | 1,940 of 1,958 | 1,874 of 1,958 |
| an operation is stated | 599 (30.9%) | 440 (23.5%) |
| determined: states nothing | 939 (48.4%) | 442 (23.6%) |
| degenerate: nothing determined | 337 (17.4%) | 270 (14.4%) |
| absent: not in the export surface | 65 (3.4%) | 722 (38.5%) |
| carries an owner requirement | 31 (1.6%) | 34 (1.8%) |
| sites whose package published no catalog | 18 | 84 |

Certification itself was healthy, which is what rules out a bad run as the
explanation: 30 probes, 30 certifications verified, 0 refused, 28 complete and
2 partial, 100 certified entrypoints, 96.67% probe success.

## Why the numbers moved, and why it is not a regression

**The denominator names one package major and the corpus installs another.**
Demand was swept over consumers written against the Solid 1.x-era releases.
A `--solid 2` run installs each package's Solid 2 release, which for a package
mid-rewrite is a different API.

The evidence is not statistical. `@kobalte/utils@2.0.0-alpha.0` declares exactly
fifteen names at `.`, of which ten are value exports, and the certified surface
holds exactly those ten. `mergeDefaultProps` does not appear anywhere in the
package. The frozen demand asks about it 254 times. Of the 41 demanded exports,
8 are present (220 sites) and 33 are absent (722 sites), including `mergeRefs`
(164), `access` (66) and `createGenerateId` (60).

So the 722 is the Solid 2 release having dropped the API, not a contract
declining to describe it. The bucket's label was written when `absent` was 65
sites and meant an entrypoint refusal; it now reads *not in the export surface*,
because that is what it measures.

**Two packages have no Solid 2 release at all.** `@solidjs/start@2.0.3` (24
sites) and `@kobalte/solidbase@0.6.13` (60 sites) are solid1-only rows, so they
publish no catalog under `--solid 2` and their 84 sites are unmeasured. In
exchange `@kobalte/core` (16) and `@solid-primitives/context` (2) became
measurable, both degenerate.

**What did improve.** Owner requirements, the only class that raises a finding
on the consumer's own code, went from 31 sites to 34, and `@solid-primitives/permission`
and `@solid-primitives/keyed` now carry them where they did not. Degenerate
sites fell in absolute terms, from 337 to 270.

## How to read this baseline

- `operations`, `closed-empty` and `degenerate` are shares of sites the corpus
  can actually be asked about. They are the coverage signal.
- `absent` on a cross-major run is a note about the ecosystem, not about this
  checker. Do not report it as contract coverage.
- The two baselines must not be differenced. The census refuses the comparison
  outright rather than annotating it, and that refusal is the point.

## The decision this leaves open

ADR 0110 § 5 froze the denominator on the argument that demand measures what
consumers call, which is a fact about the ecosystem rather than about the
dialect. That argument is sound for *counting* demand and does not survive
contact with a cross-major run: one major's demand is not answerable by another
major's artifact, which is what 38.5% `absent` records.

Two ways out, both deliberate and neither taken here:

1. **Re-sweep demand over Solid 2 consumers.** The honest denominator for a
   Solid 2 census. It is the § 5 reversal the ADR already names, and it shrinks
   the corpus, so every percentage moves again and for a second reason.
2. **Split the `absent` bucket** into "this artifact has no such export" and
   "the contract declines to describe an export it has". Only the second is a
   contract gap. This needs the artifact's export surface carried into the
   census, which the certification already knows and the run report does not
   currently record.

Until one of those lands, the Solid 2 coverage signal is the three buckets over
1,152 sites that are in-surface, not the headline share over 1,874.
