# unsupported-solid-runtime

`SC9013` · **error** · uncertifiable

The `solid-js` this project would actually import resolves to a major version
this build has no dialect for, or to a release line of a carried major that its
vocabulary refuses. The checker refuses the project rather than analyzing it
under a language or runtime it does not run.

## What it does

Before any analysis, the checker resolves the dialect from the nearest
`node_modules/solid-js/package.json` above the project, walked the way a
bundler resolves (`rust/crates/solid-facts-backend/src/dialect.rs`). That walk
has three outcomes, and only this one is a refusal:

- the installed major has a dialect in this build — analysis proceeds under it,
  unless its vocabulary refuses the exact release (below). A carried release
  the vocabulary was not audited on is analyzed with the
  [unaudited-solid-release](unaudited-solid-release.md) notice beside the
  findings;
- **the installed major has no dialect in this build — this finding, and no
  others**;
- nothing resolves, or the version names no released major (`workspace:*`,
  a prerelease of an unreleased major) — the default dialect applies, and the
  project is analyzed normally.

The finding names the exact `package.json` that decided it and the version it
read. It is the only identity in the catalog the rules engine never produces:
it is emitted at the dialect-selection site, so **no other finding accompanies
it**. An empty finding list would be a false certification, which is what this
rule exists to prevent.

Since ADR 0110 this build carries the Solid 2 dialect only, so an installed
Solid 1.x runtime is the case that reaches it most often.

### Refused releases of a carried major

The Solid 2 vocabulary refuses one release line inside its own major:
**`2.0.0-experimental.x`**, the pre-beta Solid 2 experiment. It is not an older
release candidate. It runs a different runtime, and analyzing it under the rc
vocabulary would misread the argument positions the rules rely on. The review
measured it on `2.0.0-experimental.1`
(`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`
§ 8):

- it depends on `@solidjs/signals@0.1.0`, not the `2.0.0-rc` signals;
- `createEffect(compute, effect, error?, value?, options?)` takes an error
  handler at argument 2, where the vocabulary reads options;
- `createMemo(compute, value?, options?)` takes a seed value at argument 1,
  where the vocabulary reads options;
- its boundaries are the 1.x-era `Suspense` and `ErrorBoundary`, not `Loading`
  and `Errored`, so the rules would not see them.

The finding then states that reason instead of "carries no dialect for it",
which would be false: the dialect is carried, and this line is refused.

No other pre-release line is refused. Betas, other release candidates and
`2.0.0` are analyzed with the [unaudited-solid-release](unaudited-solid-release.md)
notice. That includes an alpha, which nobody here has measured.

## Why it matters

Solid 1.x and Solid 2.0 are different languages at the seams this checker
proves things about: effect arity and return contracts, store creation, owner
and settlement vocabulary, and which module publishes `JSX`. Analyzing a 1.x
project under the 2.0 catalog does not degrade gracefully — it produces
confident findings about a runtime the project does not have, and misses the
ones it does.

The alternative to refusing is a silent wrong-language analysis, which is
indistinguishable from a correct one in every output the tool produces. So this
is an **uncertifiable** result, not a violation: the project's own source is not
the defect, and the checker asserts nothing about it.

## How to fix

Upgrade the project to Solid 2.0, or pin a checker release that still carries
the 1.x dialect. For `2.0.0-experimental.x`, move to a Solid 2.0 release
candidate; `2.0.0-rc.9` is the audited one. `docs/adr/0110-the-checker-analyzes-solid-2-only.md` records
why this build carries one dialect, and `docs/rule-catalog-migration.md` maps
the 1.x rule names onto their 2.0 identities for a project migrating its
suppressions.

If the version is a deliberate placeholder (`workspace:*`, a monorepo link),
nothing resolves to a released major and the project is analyzed under the
default dialect instead — this finding does not appear.

## Suppression

Not suppressible per site: there is no site. Passing `--dialect solid-v2`
explicitly overrides detection and analyzes the project anyway, which is
appropriate only when you know the resolved manifest misreports what will
actually be installed. It is not a way to check a 1.x project.
