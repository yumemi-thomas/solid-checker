# solid-checker

A Rust engine that proves Solid 2.0 reactivity defects, with Node (packages/cli)
and WASM (packages/wasm) adapters. Solid 2 is the only dialect: a project whose
`solid-js` resolves to another major is refused with SC9013 (ADR 0110). Use the
vocabulary in CONTEXT.md. Rule pages: docs/rules/. Design: rust/ARCHITECTURE.md.

## Invariants

1. **Never report what `tsc` reports.** If `tsc --noEmit` errors on the same code
   against the library's real published typings, the checker stays silent. No
   exceptions: not better wording, an autofix, or "the project may not be
   strict". Fixture stubs lie: a stub looser than the real package (a callback
   return typed `unknown` where the package says `(() => void) | void`) invents
   a defect no project can have, and every gate stays green. Check a rule's
   case against the published types before adding or keeping it. A rule found
   duplicating `tsc` is deleted, not demoted, and the removal is recorded in
   docs/precision-backlog.md with the `tsc` output. A different claim about the
   same code (ownership, timing) is allowed.
2. **Fail closed; zero false positives.** Report a violation only when facts and
   the execution model prove it. A missing fact, symbol, call target, compiler
   behavior, or dependency source yields an uncertifiable result (internal
   only, ADR 0328), never a violation and never a silent clean.
   "Unresolved" is not evidence of anything.
3. **Exact resolution.** Resolve exact symbols and declarations. No name-only
   matching, smallest-contained-symbol, wildcard trust, guessed member dispatch,
   or package behavior inferred from a package name. External behavior comes
   from dependency source under the documented premises (ADRs 0266, 0300);
   unavailable source is SC9005.
4. **Seams.** solid-facts owns syntax and facts; solid-facts-backend owns
   orchestration and the process boundary; solid-reactive-ir owns indexes,
   interprocedural analysis, and proof obligations; solid-dialect owns the
   dialect interface; rust/dialects/solid-v2 owns Solid 2 vocabulary and rules.
   Never pass TypeScript-Go or Oxc nodes across fact interfaces. Put
   version-specific behavior behind a `Dialect` question; a `Version` match must
   stay exhaustive. Do not grow one monolithic reactivity rule.
5. **Ported code is retained behavior.** `solid-reactive-ir/src/upstream_compat/`
   ports eslint-plugin-solid 0.14.5 (commit 6d3bc311). An odd heuristic may be
   deliberate: read upstream at that revision before changing it, and pin a kept
   divergence as a case in fixtures/ownership-cases/cases.json saying why 2.0
   wants it.
6. **Snapshots record decisions.** Run `make coverage` and read every moved
   finding before `make coverage-update`; the update goes in the same commit as
   the code that moved it.
7. **Pinned inputs.** The Solid 2 compiler is consumed at an exact
   semantic-facts-only fork revision: no lowering, output, diagnostic, runtime,
   or unrelated changes there. Package-source fixtures copy published bytes
   with package, version, and integrity recorded. Never substitute a newer
   Solid prerelease.

## Working rules

- Run `git status --short` first and preserve unrelated changes. Never run
  `git reset --hard`, `git checkout --`, or `git clean`, and never push.
- One Cargo process per `rust/target`. Parallel agents that build use their own
  worktree and target directory.
- Each semantic branch gets a positive fixture, a negative fixture, and the
  shadowed, namespace, member, or wrapper variant when that is the distinction.
  Authoring fixtures: `.claude/skills/add-fixture/SKILL.md` (read it first; a
  fixture's `node_modules/solid-js` stub needs `.gitignore` exception lines or
  it silently becomes a no-op).
- Search code first: `rg SYMBOL rust packages scripts`. `.ignore` hides the
  precision backlog, contract audits, snapshots, and fixture node_modules; use
  `rg --no-ignore` when you need them.

## Checks

- Iterate on one Rust test: `make test-focused TEST=<filter> TEST_PACKAGE=<crate>`
  (library tests only).
- Process tests skip silently unless armed:
  `SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test <name>`.
- After a slice: `make verify-delta BASE=<commit the slice started from>`. It
  maps changed paths to their owning checks plus fmt, clippy, and manifest
  validation, and escalates any unmapped path to `make verify`.
- Fixture or finding changes: `make coverage`, then `make ownership-gate`. Both
  build and use the fresh debug checker.
- Handoff authority: `make verify`, once, at the end.
- `make` targets rebuild `bin/solid-typefacts` when its source stamp is stale;
  bare `cargo` does not. If a gate result looks impossible, rerun it with
  `SOLID_CHECKER_GATE_CACHE=0`.
- Sweeps and timing use the release binary (`make build-checker-release`); the
  debug binary inflates certification about 19x.

## Report

State what changed, which checks ran with their results, which were deferred,
and the remaining uncertifiable cases. Do not claim more than was verified.
When precision status changes, update the rule page or add a short entry at
the top of docs/precision-backlog.md (2 MB: read only the first lines).
Other procedures: `.claude/skills/verify-handoff`, `green-commits`.
