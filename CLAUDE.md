# Claude instructions for solid-checker

@AGENTS.md

AGENTS.md above is canonical and complete — follow it fully. The notes below
are the only Claude-specific deltas; do not treat them as a summary of
AGENTS.md.

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
