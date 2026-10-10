# ADR 0187: Certifications under one condition set are not ambiguous

- Status: accepted and implemented (2026-10-05). Eleventh lever of the owner's
  package-misuse goal of 2026-10-04.
- Owners: case selection in admission (`select_declared_cases` in
  `contract_interface.rs`), shared by the compiled-in tier and project
  catalogs through `admit_by_artifact`.
- Relation: refines ADR 0140's case selection. The admission checks before
  it (acceptance root, installed bytes, installed environment) are unchanged.

## Context

After ADR 0186 bundled the 38-app certifications, `viviana-ui-web`'s 213
imports of `@tanstack/solid-router` were still reported as having no accepted
contract, although its own in-place certification was in the tier. An
instrumented run showed why:

- five rc.8 bundles authenticated in its tree, three of them for
  `[browser, import]`;
- all three reached the file the project resolved (`dist/esm/index.d.ts`);
- case selection found three equally specific applicable cases and refused
  them as ambiguous.

The three are certifications of the same artifact case under the same
condition set: one made in this app (shared with `finds-team`), two carried
from earlier runs. The refusal was meant for cases under *different* condition
sets, which a declaration cannot choose between. Every certification of one
package version under one condition set, in equivalent trees, refused all the
others. So the more consumer-environment bundles the tier carried, the less
it delivered.

## Decision

1. **Different condition sets still refuse.** Two equally specific
   applicable cases under different sets select nothing, as before.
2. **One condition set selects one case.** Every candidate has passed the
   acceptance-root, installed-bytes and installed-environment checks. Each
   one's claims were therefore proven for an environment this tree installs
   in full, and keeping any of them is sound. They are not merged: they may
   state different claims, since they may come from different certifier
   builds or close different domains. The case kept is the one whose
   recorded environment has the most entries (the most premises checked
   against this tree), then the smallest identity, so every run keeps the
   same one.

## Consequences

- An undeclared host (ESLint, Oxlint) is unaffected: it still receives every
  host-free candidate, and the agreement check decides.
- The case kept may close fewer domains than another candidate would.
  Choosing the strongest contract would need the contracts' content at
  selection time; this rule only guarantees a sound one.

## Evidence

- Unit test `same_set_certifications_keep_the_most_premised_one`: three
  same-set cases keep the most-premised one, ties broken by smallest
  identity; two equally specific cases under different sets still select
  nothing. The other 16 case-selection tests pass unchanged.
- `viviana-ui-web` on the plain release binary: `@tanstack/solid-router` goes
  from "no accepted reactivity contract" (213 import sites) to admitted. Its
  partial contract now reports per-export open claims, which is exactly what
  its own project catalog gave.
- 38-app browser sweep: violations unchanged at 273. Uncertifiable goes from
  10,355 to 10,377 (+24, −2): viviana's router imports now carry a partial
  contract instead of none. The node-mode sweep is unchanged.
- Misuse ledger: unchanged at 79 static violations of 123, and no correct
  twin is flagged.
- Coverage and the ownership gate are unchanged. `make test-rust` passes except
  the known pre-existing
  `sessions_process::incremental_contract_exports_refresh_changed_summaries`.
