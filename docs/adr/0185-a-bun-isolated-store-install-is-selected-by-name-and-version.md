# ADR 0185: A Bun isolated-store install is selected by name and version

- Status: accepted and implemented (2026-10-05). Ninth lever of the owner's
  package-misuse goal of 2026-10-04; the second aimed at real app installs.
- Owners: Bun lock selection for an installed package
  (`bunLockSelectionForInstalledPackage` in
  `packages/cli/scripts/published-contract-graph.mjs`), used by
  `exactLockSelection` and the Bun graph walker.
- Relation: the Bun counterpart of pnpm's `name@version` key. No wire change
  and no new trust: the record selected is still the lockfile's own, with its
  own integrity.

## Context

Bun's isolated linker installs every package once, at
`node_modules/.bun/<name, "/" as "+">@<version>[+<peer hash>]/node_modules/<name>`,
and links it into each workspace. A Bun lock key, though, is the hoisted
install path (`@tanstack/solid-query`, `parent/leaf`). Selection derived the
key from the install path, so an isolated-store copy produced
`.bun/@tanstack+solid-query@…/@tanstack/solid-query`. That matched no record,
and certification refused with:

> missing-lock-selection: @tanstack/solid-query@6.0.0-rc.0 has 0 exact Bun selections

Three of the 38 apps use this layout (`compass-ui`, `skyjtx-website` and
`jibe-run`). None of their 16 packages certified.

## Decision

1. **An isolated-store install is recognized by its path.** The root (or its
   real path) is `node_modules/.bun/<store>/node_modules/<name>` beside the
   lockfile. `<store>` is exactly `<name>@<version>`, or that followed by
   `+<peers>`, and `<name>` is the package's.
2. **Its record is the one its name and version agree on.** Every record at
   `name@version` must carry an integrity, and all integrities must be equal.
   Then they all name the same published bytes, and the first record by
   locator is selected, so a certifier and an admitting consumer select the
   same one. Records that disagree stay `ambiguous-lock-selection`, because
   the store path does not say which copy it holds.
3. Hoisted installs keep the exact path-derived key, unchanged.

## Consequences

- No new trust: the integrity is the lockfile's, and the archive is still
  authenticated against it.
- The refusal wording `has N exact Bun selections` is shared with the pnpm and
  npm paths, which call the same function. An npm lockfile that records no
  `integrity` for a package (`ai-memory-ui`, 140 of 381 entries) is still
  refused, correctly: there is nothing to authenticate against.

## Evidence

- CLI test `an isolated Bun store install selects the record its name and
  version agree on`:
  - plain and scoped store paths, with and without a peer hash, are
    recognized;
  - another version, another name, a hoisted path or a store outside the
    lockfile's tree are not;
  - agreeing records select one; disagreeing integrities stay ambiguous.

  The whole CLI suite passes (378 tests).
- The three isolated-store apps go from 0 certified to 4 of 7 (`compass-ui`),
  3 of 5 (`skyjtx-website`) and 4 of 4 (`jibe-run`). Their violation counts are
  unchanged.
- 38-app local certification rerun (`rust/target/local-certify/local-0185`):
  certifications go from 100 to 111 of 180. Violations stay at +9/−1 against
  the uncertified baseline, with no false positive.
- A read-only diagnosis by a delegated agent
  (`rust/target/local-certify/sol-diagnosis.md`) traced the `@solidjs/router`
  and `@solidjs/meta` refusals in these apps to the same missing declaration
  sources. Their accepted contracts are mostly `call: {}`, though, so
  admitting them raises coverage, not findings.
