# Claude instructions for solid-checker

@AGENTS.md

AGENTS.md above is canonical and complete — follow it fully. The notes below
are the only Claude-specific deltas; do not treat them as a summary of
AGENTS.md.

## Absolute rule, restated because it overrides every instinct to add a rule

**Never report what TypeScript already reports.** If `tsc` errors on the same
code against the library's *real published typings*, this checker must stay
silent. AGENTS.md carries the full rule and its corollaries — read it there
before adding, keeping, or "improving" any rule.

The one mechanic worth repeating, because it is how this rule gets broken
without anyone noticing: **fixture stubs lie**. A stub typing a callback return
as `unknown` where the real package says `(() => void) | void` invents a defect
that no real project can produce, and every gate stays green while the rule
duplicates `tsc`. Before you trust a rule, write its case against the published
types and run `tsc --noEmit`. A rule that only survives against a loosened stub
is not a rule.

## Tool mapping

- Where AGENTS.md says `apply_patch`, use the Edit/Write tools.
- Run only one Cargo build/test/clippy process at a time **per target
  directory**; parallel Cargo commands on one `rust/target` contend for its
  build lock. Parallel subagents that build therefore each work in their own
  git worktree with their own `rust/target`, never in the shared checkout.
  Seed a new worktree's target with an APFS clone of the main one, leaving out
  `debug/incremental` (116 GB, and Cargo rebuilds it): `cp -cR` of
  `rust/target/release` and of every `rust/target/debug` entry except
  `incremental`, plus `bin/solid-typefacts{,.buildinfo}` and
  `packages/cli/probe-harness.buildinfo`; symlink the `node_modules`
  directories. The lead merges the worktree's commits. An agent spawned with
  worktree isolation starts from `main`, not from the lead's branch: its first
  step is `git switch -c <branch> <the lead's exact commit>`.
- Sweeps and census-style measurements use the release binary
  (`make build-checker-release`, about 45 s incremental): the debug binary
  inflates certification about 19x and a consumer sweep from seconds to
  minutes.

## Skills

Invoke the matching repo skill before starting these task types; each one
carries verified procedure and traps that are not repeated in AGENTS.md:

- `verify-handoff` — choosing checks proportional to a change and writing the
  final report.
- `add-fixture` — authoring semantic fixtures, dialect stubs, and snapshot
  updates.
- `upstream-parity` — investigating parity divergences against
  eslint-plugin-solid.
- `green-commits` — slicing a large worktree into individually green commits.

## Vocabulary

Use the canonical terms from CONTEXT.md (fact domain, finding kind,
uncertifiable, failure class, …) in code, findings, and reports; each entry
lists spellings to avoid.
