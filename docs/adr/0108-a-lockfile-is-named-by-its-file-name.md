# ADR 0108: A lockfile is named by its file name, and pnpm is read

- Status: accepted and implemented (2026-09-14); written with the
  implementation
- Date: 2026-09-14
- Owners: published-graph acquisition
  (`packages/cli/scripts/published-contract-graph.mjs`) and its authority
  (`contract_certification/dependencies.rs`)
- Relation: no handshake protocol change and no wire field. The lockfile *path*
  already travels to Rust; its file name is the format decision on both sides.

## Context

Policy-2 certification refused any package not installed under a Bun text
lockfile. Measured on 2026-09-14, that refusal is load-bearing in the wrong
place: the whole demand corpus — `solid-primitives`, `corvu`, `kobalte`,
`solid-docs`, 146 real consumer projects — installs with pnpm, so the path that
would let any of them accept a certified contract could not run at all. The
lane worked only for the ecosystem benchmark's own Bun scratch projects.

What the lockfile supplies is narrow. For one `name@version` it names an
integrity and a locator, and nothing else; installed bytes are never evidence,
because the caller acquires the package from the registry and Rust re-derives
the snapshot from those archive bytes. Every lockfile format records that much.

## Decision

**The lockfile's file name is its format**, on both sides of the trust
boundary. `bun.lock` is read by the Bun reader, `pnpm-lock.yaml` by a new pnpm
reader, and any other name is refused. No format tag is added to the wire, and
neither side sniffs content.

Two consequences are deliberate:

- A directory holding two lockfiles is **refused**, not ordered by preference.
  Which one installed that tree would be a guess, and a guess here selects
  which published artifact is fetched.
- Only pnpm lockfile **major 9** is read. This is the premise the rest rests
  on, not conservative packaging — see below.

## Why pnpm needs no installed-path locator

Bun derives a locator from the install path because its tree is where the
identity lives: one `name@version` can sit at two paths with *different*
integrity, which is exactly what
`bun_lock_selection_uses_the_installed_locator_to_disambiguate_same_versions`
pins.

pnpm cannot do that. Its store is content-addressed, its `packages:` keys are
exactly `name@version`, and each carries one integrity; peer resolutions live
in `snapshots:` and share the tarball. So the locator is the key itself, and a
same-version duplicate is a **refusal** rather than a selection to
disambiguate.

Measured across the three lockfiles of the demand corpus (3,871 packages):
zero duplicate keys, zero entries without integrity, zero peer-suffixed keys in
`packages:`, zero non-registry entries.

That evidence is for major 9 only. Major 6 wrote peer suffixes into the
`packages:` keys themselves (`foo@1.0.0(bar@2.0.0)`), so one `name@version`
could appear under several keys with no installed path to separate them, and
exact selection would not be decidable. Refusing the major is what makes "the
key is the locator" sound; it is not a packaging convenience.

## Why a strict subset reader rather than a YAML parser

The reader is written twice — Node acquires, Rust authenticates — and both
implement one block of one lockfile major. Anchors, aliases, merge keys, tabs
and second documents are **refused**, not ignored: each can move a value from
one entry to another or redefine `packages:` wholesale, so a reader that
skipped what it did not understand would answer confidently from the wrong
bytes. A general YAML parser in the authority path would have to accept all of
them.

Two shapes had to be learned from the corpus rather than from pnpm's writer, and
both were caught by reading real lockfiles rather than by reasoning:

- The `solid-docs` lockfile has been through a formatter, so its keys are
  double-quoted and `resolution` is a multi-line flow mapping with a trailing
  comma. A line-shaped reader answers "no packages" for it — fail-closed, but
  for the wrong reason, and the graph then refuses a package that is perfectly
  well locked.
- `specifier: workspace:*` appears in every lockfile in the corpus. A `*` opens
  a YAML alias only at a token boundary, and a bare `:` is not one:
  `workspace:*` is a single plain scalar. Both readers first treated the colon
  as a boundary and so refused **every** real lockfile.

That second one is the argument for writing the two readers against the same
pinned roster. They disagreed first — the acquisition side required a character
after the `*`, the authority side did not — and the divergence was invisible end
to end, because the lane that certifies a root package never reaches the Rust
reader at all. A subset one side reads and the other refuses turns an unsound
lockfile into a confusing late refusal instead of an early, accurate one, so the
rosters are mirrored case for case.

## Consequences

- A pnpm-installed project can certify and accept a contract. Verified against
  the real `pnpm-lock.yaml` with no `bun.lock` present:
  `@solid-primitives/utils@6.4.1` issued a receipt into
  `kobalte/packages/core/.solid-checker/accepted-contracts.json`, and its
  graph lane closed 61 entries.
- **Which side that run exercised, precisely.** Certifying a root package reads
  the lockfile in Node only; the Rust reader is reached from
  `certification_graph_node_from_request` and nothing else. An A/B with the
  authority's token rule deliberately broken certified the same package
  unchanged, which is how the divergence above stayed invisible. The Rust
  reader is therefore covered by
  `a_graph_node_reads_a_pnpm_lockfile_named_by_its_file_name`, which drives the
  real request path and asserts the pre-9 refusal only that reader emits — not
  by any end-to-end certification run.
- The Bun lane is unchanged; its reader, locator derivation and tests are
  untouched.
- `PublishedGraphLockSelection` already carried `package_manager` and never
  compared it to `"bun"`, so a pnpm-sourced node simply records a different
  provenance in its identity digest.
- Yarn Berry is **not** read. Its `checksum` is a zip content hash rather than a
  registry tarball integrity, so it cannot feed this path without a conversion
  this ADR does not attempt.
- What this does **not** change: no closure, no census, no claim domain. It
  removes *one* reason a consumer could not accept a contract. Measured
  immediately afterwards, a second one stands: the acceptance index is keyed on
  `(importer, specifier)`, and certification binds its acceptance to a synthetic
  importer it writes inside the package, so accepting a certified contract still
  moves no consumer finding. See § 7 of
  `phase21/2026-09-14-which-closures-change-a-consumer-finding.md`.
