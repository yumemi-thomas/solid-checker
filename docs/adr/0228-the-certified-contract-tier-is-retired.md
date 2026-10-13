# ADR 0228: The certified contract tier is retired

- Status: accepted and implemented (2026-10-08).
- Owners: the shared admission rule (`solid-facts-backend/src/artifact_admission.rs`,
  formerly `accepted_bundles.rs`); contract acquisition in `diagnostics.rs`;
  plain-lane finalization (`contract_certification/finalization.rs`);
  `scripts/author-contracts.mjs` and every authored spec's `identity.json`;
  the WASM request (`solid-checker-wasm/src/lib.rs`).
- Relation: supersedes ADRs 0186 and 0191, and ADR 0151's compiled-in
  citations. Supersedes the compiled-in tier parts of ADRs 0123 and 0126.
  Amends ADR 0198's precedence and bootstrap, and ADR 0207's identity
  source. Project-catalog admission, receipt authentication and the authored
  tier stay in force.

## Context

The checker carried a certified tier: `pkg/contracts/accepted/`, 1,876
bundles for 126 packages, about 15 MB, compiled into every build. No bundle
was tied to the audited rc.13 runtime: 964 were tied to rc.9, the rest to
rc.0 through rc.8.

Since ADR 0198, the authored tier supplies the package claims that move
findings. It states its package bytes and its Solid runtime, and every claim
has a probe. The certified tier still cost its index, its objects, its
regeneration workflow, a citation lookup, and a second environment rule
(ADR 0191).

An A/B run used the same release binary, with only the certified tier
turned off:

| Measure | Tier on | Tier off |
| --- | --- | --- |
| rc.13 corpus sweep: violations | 235 | 235 (0 added, 0 removed) |
| Base app-patterns ledger | 29 proven / 31 detected | identical |
| Corpus twins | 25 proven / 13 uncertifiable | identical |
| Primitives ledger (123 cases) | — | identical |

The only corpus difference: six `vite.config.ts` notices move from column 8
to column 1. They come from the nine runtime-free bundles
(`@solidjs/vite-plugin` 3.0.0-next.34, next.35 and next.36,
`vite-plugin-solid-svg`, `@solid-primitives/input-mask` 1.0.0-next.2).

## Decision

1. **The certified tier is deleted.** That covers `pkg/contracts/accepted/`,
   its embedded list, its loader, `scripts/bundle-accepted-contracts.mjs`,
   the singular `solid-contract-bundle` bin with `contract_bundling.rs`, and
   `make accepted-bundles`. Precedence is now project catalogs, then the
   authored tier.
2. **The admission rule stays, renamed.** `accepted_bundles.rs` becomes
   `artifact_admission.rs`. It keeps `admit_by_artifact`, the environment
   replay, the patch and snapshot checks, `AdmissionRefusal` and
   `environment_acceptance_identity`. Project catalogs and the authored tier
   use it. `EnvironmentRule::SolidRuntime` served only the certified tier
   and is gone; every acceptance now reproduces its whole stated environment.
3. **Compiled-in citations are retired.** The native `--cite-compiled-in`
   query and its Node caller in `generate-package-contract.mjs` are removed
   together. A receipt that cites a compiled-in acceptance is refused with
   `CitationWithdrawn`: no build carries the claim it rests on. A plain-lane
   certification whose closure names a dependency edge now refuses with
   `DependencyAuthorityMissing`, naming the edge. Graph transactions still
   supply their own dependency receipts.
4. **Every authored spec carries its own identity.** The 19 specs that read
   their artifact cases from the certified index now carry `identity.json`,
   in ADR 0207's format. The authored index, objects and `embedded.rs` came
   out byte-identical. The four virtual-core property-get digests hash the
   identity cases, so those pairs were re-probed: same verdicts, new digests.
5. **WASM keeps its request shape.** `bundledContracts`, `installedPackages`
   and `exportConditions` are still accepted and have no effect. The authored
   tier is not served there: it admits on a Solid runtime environment, and a
   host with no filesystem can show only an empty one. Host-supplied
   `acceptedContracts` is unchanged.
6. **Kept:** `pkg/contracts/bundled/` and the plural
   `solid-contract-bundles`, project catalogs and `solid-contract-authorize`,
   `proof_policy_2` and receipt authentication, and the IR's
   `AcceptedContractIndex`.

## Consequences

- A project gets no contract from the retired set. An import with no
  authored entry and no admitted project contract stays uncertifiable.
- Older runtimes (rc.0 through rc.9) lose their only shipped supply. Those
  rcs already carry the SC9014 notice and get no new work.
- A project catalog whose receipt cites a compiled-in acceptance is refused
  and needs a new certification.
- `scripts/primitives-checkpoint.mjs` and `scripts/app-import-metric.mjs`
  read the authored index for their tier criterion.
- The A/B result holds for the rc.13 corpus and the named ledgers. It does
  not show that a project matching an old certified bundle keeps its
  findings.
