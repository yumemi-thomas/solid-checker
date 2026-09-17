# unsupported-solid-runtime

`SC9013` · **error** · uncertifiable

The `solid-js` this project would actually import resolves to a major version
this build has no dialect for. The checker refuses the project rather than
analyzing it under a language it does not run.

## What it does

Before any analysis, the checker resolves the dialect from the nearest
`node_modules/solid-js/package.json` above the project, walked the way a
bundler resolves (`rust/crates/solid-facts-backend/src/dialect.rs`). That walk
has three outcomes, and only this one is a refusal:

- the installed major has a dialect in this build — analysis proceeds under it;
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
Solid 1.x runtime is the case that reaches it.

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
the 1.x dialect. `docs/adr/0110-the-checker-analyzes-solid-2-only.md` records
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
