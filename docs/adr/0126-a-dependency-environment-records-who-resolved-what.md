# ADR 0126: A dependency environment records who resolved what

- Status: superseded in part by [ADR 0228](0228-the-certified-contract-tier-is-retired.md) (2026-10-08): compiled-in tier delivery and regeneration are retired. Dependency-environment edges and their replay remain in force. Originally: accepted and implemented (2026-09-26); written with the implementation
- Date: 2026-09-26
- Owners: the certifier's source collector (`certify-contract.mjs`), environment
  assembly (`contract_certification/environment_edges.rs`, `finalization.rs`,
  `dependencies.rs`), the receipt's environment root (`policy2_receipt.rs`),
  admission (`accepted_bundles.rs`, `diagnostics.rs`), and the bundler
  (`scripts/bundle-accepted-contracts.mjs`)
- Relation: amends ADR 0123 and ADR 0125. Their admission rule stays in force
  for environments without edges. Their statement that an environment naming
  two copies of one package never applies no longer holds for edged
  environments.

## Context

ADR 0123 admits a contract only where the consumer's tree reproduces the
receipt's signed environment, a set of `{name, version, integrity}`. With no
record of who looked each entry up, admission had to require that every lookup
of every name, from every located package, reach that entry.

Measured on kobalte core (pnpm), 2026-09-26: `vite-plugin-solid@3.0.0-next.5`
was refused in the very tree it was certified in, with "merge-anything
installed 6.0.6, certified 5.1.7". Its own `merge-anything` is 5.1.7, but
pnpm's hoisted `.pnpm/node_modules` exposes 6.0.6 to the other located
packages, none of which depend on it. `solid-refresh` hit the same thing.
Certify exited 0 without a word. The rule was sound, but it refused any pnpm
tree holding two versions of a package in the environment.

## Decision

**Each environment entry records the lookup that reached it:
`resolvedFrom: {importer, specifier}`.** The importer is `"certified"` (the
package itself) or another entry's `{name, version, integrity}`.

- An environment is edged on every entry or on none, and every entry must be
  reachable from `certified`.
- One name may appear more than once, under different importers.
- Edged environments hash under a new root domain,
  `solid-checker:policy2-dependency-environment:edges:v3`, which covers every
  edge field. Environments without edges keep their encoding, byte for byte.

**Admission replays each edge from its importer.** The lookup runs from the
imported package's real path for `certified`, and otherwise from every location
an earlier edge reached the importer at. It must reach exactly the entry.
Lookups the certification never made are not asked, so a version hoisted for
unrelated packages is irrelevant. Missing, unresolvable, mismatched or
unreached still refuses, and the reason names the edge, for example
"merge-anything resolved from vite-plugin-solid@3.0.0-next.5 installed 6.0.6,
certified 5.1.7".

**Environments without edges keep ADR 0123's all-lookups rule**, which is
sound for them. So the 209 compiled-in bundles still work unchanged. When the
certifier cannot give every entry an edge (a graph leaf whose environment is
graph-wide, or an older adapter), it states the environment without edges
rather than guessing.

**Certify checks its own admission.** After publishing, it replays admission
steps 1-3 for each entry it issued, in the project that holds the catalog (or
from the package root). A refusal exits 1 with "certified but not admitted:
<reason>", and the audit records `selfAdmission`. Step 4 (the resolved file
and case selection) needs a project's imports, so a certification cannot
check it.

## Consequences

Measured:

- coverage unchanged (87 projects, 452 findings);
- a pnpm-shaped process test: the edged catalog is admitted in its own tree,
  the same packages stated without edges are refused with the hoisted-version
  reason, and a repointed own copy is refused naming the edge;
- end to end, in a copy of the kobalte tree: `vite-plugin-solid` gets 16 edged
  entries (`@babel/core`, `@babel/types` and `vite` under two importers each),
  exits 0, self-admission agrees, and `contract check` reports `certified`.
  Repointing its own `merge-anything` at the hoisted copy gives `missing` with
  the edge named.

To give existing contracts edges they must be certified again: project
catalogs with `contract certify`, and the compiled-in tier with a new
certification run and regeneration.

Still open:

- lookups are replayed from package roots, not from the importing file's
  directory;
- a graph leaf's graph-wide environment usually falls back to the strict form.

## Amendment (2026-09-27): ADR 0131

An edge that reaches exactly its entry by identity still refuses when the tree
records the installed copy as patched. The reason names the edge, for example
"leaf-dep@1.0.0 resolved from @solidjs/signals@2.0.0-rc.6 is patched
(pnpm-lock.yaml patchedDependencies)". Self-admission applies the same rule, so
`contract certify` in a patched tree exits 1.
