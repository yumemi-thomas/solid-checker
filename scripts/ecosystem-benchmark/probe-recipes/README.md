# The ecosystem rows' probe-recipe corpus

Hand-authored, claim-addressed runtime-probe recipes for the ecosystem
benchmark's real rows. Supply it with

~~~sh
bun scripts/ecosystem-benchmark/run.mjs --attempt-certification \
  --probe-recipe-corpus "$PWD/scripts/ecosystem-benchmark/probe-recipes" …
~~~

A corpus is an **input, never a root of trust** (ADR 0006 § Recipes,
CONTEXT.md § Recipe corpus). Omitting a scheduled gate refuses that gate; a
vacuous recipe only fails to veto and can never establish closure, which
remains the implementation census's job (ADR 0008). Rust derives every module's
construction digest from the bytes it copied, so this directory cannot present
one identity and run another.

## Why here

The runner's other hand-authored, checked-in inputs — `manifest.json`,
`sentinel.json`, `phase16-thresholds.json` — live in this directory, and the
runner is this corpus's only consumer. `benchmarks/ecosystem/` was rejected: it
holds the run's *outputs* (`report.json`, `report.md`), which are pinned by the
phase-20/21 ledgers, and an input under an output directory invites both to be
regenerated together. `fixtures/` was rejected too: every corpus fixture there
is scanned by `scripts/contract-corpus.mjs` and `scripts/coverage.mjs`, and
these recipes address published ecosystem packages rather than a fixture the
repository owns.

## Claim ids are content digests, and that is the corpus's weakness

A `claimId` binds the exact normalized claim — package, version, artifact case,
export, domain. Anything that moves the emitted contract document for one of
these packages moves the id, and the recipe then addresses nothing: the
candidate is *withheld* ("no recipe in corpus") and the row certifies with the
domain open, rather than failing loudly. Re-read the ids from a run's
`…certification-audit.json` `withheldClosures` array after any generator change.
Addressing that survives contract edits is ADR 0006 Stage 3.

## The rule nothing here can check

**A recipe must never hand `session` or `harness` to the package under test.**
The transcript API is the one legitimate path by which package code could reach
the transcript. None of these modules does; nothing detects it if one starts.

## What each recipe is for, and what it measured (2026-09-04)

Supplying this corpus **refuses** all three rows rather than certifying them,
and that is the honest state — see
`docs/package-contract-v2/phase21/2026-09-04-first-real-creates-certification.md`.

| recipe | claim | measured outcome |
| --- | --- | --- |
| `kobalte-utils-noop.mjs` | `@kobalte/utils@0.9.2` `noop`, `./src/noop.ts` | the implementation census **proves** `creates: []`; the probe gate then refuses because the artifact case is a `.ts` file under the private workspace's `node_modules` and the pinned interpreter will not strip types there |
| `solid-primitives-i18n-scoped-translator.mjs` | `@solid-primitives/i18n@2.2.1` `scopedTranslator`, `dist/index.js` | census refuses: uncensused invoking form `coercion (TemplateExpression)` |
| `solid-primitives-i18n-flatten.mjs` | `@solid-primitives/i18n@2.2.1` `flatten`, `dist/index.js` | census refuses: `iterationReachability` control-flow marker at depth 0 |
| `solid-primitives-i18n-chained-translator.mjs` | `@solid-primitives/i18n@2.2.1` `chainedTranslator`, `dist/index.js` | census refuses: `iterationReachability` control-flow marker at depth 0 |

`scripts/ecosystem-probe-recipes.test.mjs` pins the manifest's shape, that every
declared module exists and exports `runProbeSession`, and that no module names
`session` or `harness` in a position that hands either to a package.
