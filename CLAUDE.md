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
- Run only one Cargo build/test/clippy process at a time; parallel Cargo
  commands contend for the build lock. Do not parallelize them across
  subagents either.

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
