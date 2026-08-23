---
name: add-fixture
description: Author or modify semantic fixtures in solid-checker — fixture anatomy, dialect selection stubs, .gitignore exceptions, and the snapshot review-then-update flow. Use when adding a fixture, changing expected findings, updating findings snapshots, or when a fixture mysteriously produces no findings.
---

# Adding or changing a semantic fixture

A fixture isolates exactly one semantic claim. Before writing one, read the
README.md of the nearest existing fixture in the same group and mirror its
shape.

## Anatomy

Coverage (`scripts/coverage.mjs`) discovers every directory holding a
`tsconfig.json` under `fixtures/reactive-ir/` and `fixtures/engine/`. A fixture
directory contains:

- source files (`App.tsx`, `source.ts`, …) exercising the claim;
- `tsconfig.json` — required; it is what makes the directory a fixture;
- `README.md` stating the semantic claim and, for differential-dialect
  fixtures, why the dialects intentionally differ;
- declaration stubs (`solid-js.d.ts` or similar) so symbol resolution is
  explicit rather than accidental;
- optionally `node_modules/solid-js/package.json` — see dialect selection.

Required cases for a new semantic path:

- a positive case that must be diagnosed or certified;
- a negative case that must remain clean;
- an unresolved / shadowed / generic / namespace / member / wrapper case when
  that distinction is part of the behavior;
- where applicable, an assertion distinguishing a proven violation from an
  uncertifiable result.

## Dialect selection (trap)

The checker resolves the dialect from the nearest
`node_modules/solid-js/package.json` above the project, walked like a bundler
(`rust/crates/solid-facts-backend/src/dialect.rs`):

- version resolves to 1.x (e.g. `{"name":"solid-js","version":"1.9.14"}`) →
  the v1 rule catalog runs;
- stub missing, unparsable, or version unclassifiable (`workspace:*`) → silent
  fallback to the v2 default.

So a v1 fixture without its stub is a **no-op that still passes**. And because
`.gitignore` blocks `**/node_modules/` globally, every fixture stub needs its
own exception lines:

```
!fixtures/<group>/<name>/node_modules/
!fixtures/<group>/<name>/node_modules/**
```

Without them the stub exists locally but is silently excluded from `git add`,
and the fixture un-dialects only in CI. Verify with
`git status --short fixtures/<group>/<name>/` that the stub shows up.

Where 1.x and 2.0 intentionally differ, the behavior is pinned by the
`fixtures/reactive-ir/dialect-solid-1x` / `dialect-solid-2` pair — read those
fixtures' comments before mirroring behavior across dialects.

## Snapshot flow

Snapshots live in `fixtures/findings-snapshots/<group>__<name>.json`, one file
per fixture project, holding rule, code, kind, severity, path, and span
(messages excluded, except for the wording-under-test projects listed in
`scripts/coverage.mjs`).

1. After Rust source changes, compare with the **fresh debug binary** so a
   stale checked-in binary cannot hide the change:

   ~~~sh
   SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
   SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs
   ~~~

2. Review the reported diff. Every moved finding must be explainable by the
   intended semantic change — a snapshot update is a record of a deliberate
   change, never a way to discover what the implementation does.
3. Only then rerun with `--update`. Update only the snapshots your change
   owns; do not rewrite unrelated ones to make the run green.
4. If precision status changed (new proof, new approximation, new fail-closed
   path), record it in `docs/precision-backlog.md`.
5. Commit the snapshot update in the same commit as the code that moved the
   findings, not a thematically nearby one.

Package-contract fixtures (`fixtures/package-contracts/`) additionally pin the
exact package artifact; keep unknown external behavior fail-closed rather than
adding blanket trust to make a case green. They are registered by name in
`scripts/contract-corpus.mjs`, which compares `expected.json` (the contract) and,
where the fixture carries one, `expected-generation.json` (the review plan's
attested closure record — per entrypoint: `targets`, package-relative module
paths, `notes`, `runtimeNotes`; hashes deliberately unpinned). Add the second
file only for a claim about which modules the *analyzing program* reported it
opened: that is invisible in the contract document and cannot be tested from
`scripts/contract-generation.test.mjs`, whose stub native checker resolves
nothing. See `fixtures/package-contracts/torture-corpus.md`.

Two closure shapes cannot be a committed fixture at all: a symlink escaping the
package root (it would be this repository's first committed symlink, and arrives
as a plain file on a Windows checkout) and one file reached by two case
spellings. Those are generated into a temporary directory against the real
producer in `scripts/contract-closure-record.test.mjs` — the right home for any
closure property that depends on the filesystem underneath the package rather
than on its contents.
