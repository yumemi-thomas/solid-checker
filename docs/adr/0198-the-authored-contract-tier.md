# ADR 0198: The authored contract tier

- Status: accepted (2026-10-05); implemented in slices, see *Implementation
  status*. Track B of `docs/2026-10-05-package-direction.md`, implementing
  ADR 0189.
- Owners:
  - `accept_authored` in `solid-reactive-ir/src/contract_semantics/proof.rs`;
  - `solid-facts-backend/src/authored_contracts.rs` (the tier, its admission
    and its index);
  - `pkg/contracts/authored/` (documents and index);
  - the per-claim probe gate (slice 3).
- Relation: the first implementation of ADR 0189. The certified tier (ADR 0186)
  and project catalogs are unchanged and keep their own rules.

## Context

On the rc.13 corpus the certified tier reaches no app. Its bundles were proven
on the apps' older Solid runtimes, so the 10 package findings it delivered are
gone. The contracts behind them close no domain: each states one positive
claim (`useSearch` and `useParams` return a reactive accessor read untracked;
`useMutation` returns a store). The certified tier admits them through
built-in receipts whose bindings are mostly certification facts (demand graph,
producer sessions, witness roots, dependency receipts), which an authored
document cannot state honestly.

## Decision

1. **An authored contract is a contract document plus an index entry, with no
   receipt.**
   - The document is the ordinary schema-1 contract document, one artifact
     case per file, exactly as a bundle's.
   - The index entry states: the package identity (name, version, integrity),
     the requested entrypoint and export conditions, the runtime and
     declaration targets, the published files' snapshot root, the **Solid
     runtime the claims were probed on** (`solidRuntime`: name, version,
     integrity of `solid-js`, `@solidjs/signals`, `@solidjs/web`), and the
     document's digest.
2. **`accept_authored` builds the analyzer typestate.**
   - It recomputes, as `accept_authenticated_policy2` does: the artifacts
     digest, the closure digest and the closed-claims root (which counts a
     positive-export claim, so a document that closes no domain still binds).
   - Its receipt names the authored document's digest as wire digest and proof
     root, with verifier policy `AUTHORED_POLICY` (3) and no authentication.
     The semantic identity therefore keeps it apart from every certified
     contract in caches.
3. **Admission is by package bytes and Solid runtime** (ADR 0189 § 3), through
   the one admission rule, `admit_by_artifact`:
   - the installed name, version and integrity reproduce the acceptance root;
   - the installed files reproduce the snapshot root, and nothing is patched;
   - the environment it must reproduce is exactly the `solidRuntime` entries,
     resolved from the package's own location (`EnvironmentRule::Exact` over
     those entries, edge-free);
   - the resolved file reaches the case, and case selection is unchanged.

   The rest of the dependency tree is not compared.
4. **Precedence:** project catalogs, then the authored tier, then the
   certified tier. An authored entry is keyed `authored:…`, so it never
   collides with a certified acceptance.
5. **Every claim has a probe case** (slice 3): a misuse/correct pair run in
   headless Chrome on the published package at the listed version and the
   stated Solid runtime, in the misuse ledger's harness. The misuse case must
   raise the dev diagnostic the claim implies (`STRICT_READ_UNTRACKED` for an
   untracked read of a returned accessor or store), and the correct case must
   not. A claim without a passing pair is not shipped.
6. **Documents start from the version's own identity.** For the pilot, the
   identity and case structure of each document come from the certified
   bundle the tier already carries for that exact version. Every export's
   `call` is then replaced: the authored claims for the exports the pair
   probes, and fully open (`call: {}`) for every other export. Nothing a
   certification inferred is carried without its own probe.

7. **The tier is built from specs by `scripts/author-contracts.mjs`.**
   - A spec is `pkg/contracts/authored/specs/<package>@<version>/`: a
     `spec.json` naming the package version, the Solid runtime (with
     integrities), and per claimed export the authored `call` and the misuse
     rule its pair exercises, plus `<export>.misuse.tsx` and
     `<export>.correct.tsx`.
   - `probe <chromium> --only <spec> --install <dir>` runs the pairs through
     the misuse ledger's harness (`solid-checker feedback run`) in an install
     that holds the package. It first refuses an install whose files do not
     reproduce every certified case's artifact digest, or whose Solid runtime,
     resolved from the package's own directory, is another release. Results go
     to `pkg/contracts/authored/probe-results.json`.
   - `build` writes the documents, the index and `embedded.rs`. A claim ships
     only with a `passed` result on the spec's runtime. A document's case
     carries only identity fields (artifact, declarations, resolution); any
     other case field is refused rather than carried unprobed. Summary ids are
     readable (`authored-<export>`, `open-<shape>`), since decoding does not
     require content-addressed ids for a one-case document.
   - `check` (in `make verify` and `make contract-conformance`) fails when the
     written tier is not what `build` writes.
8. **The development collector reads rc.13.** `feedback run` instruments the
   shared reader of `@solidjs/signals/dist/dev-shared.js` from a byte-pinned
   profile. RC.13 gets its own profile beside RC.9's:
   - `getObserver` is byte-identical;
   - `read` adds the post-await read check, the derived-override guard, and an
     earlier strict-read warning;
   - `untrack` counts its depth.

   None of these changes the parameter, the returns, or the body that the
   instrumentation wraps.

## Consequences

- A contract reaches every project that installs the listed version, on the
  Solid runtime it was probed on.
- A project on another Solid release gets no authored contract, and that is
  visible: the import stays uncertifiable, under the release notice where
  applicable.
- A wrong claim is a false positive. The probe pair is the defense, and it is
  only as strong as the pair: a pair must exercise the behavior the claim
  asserts.

## Implementation status

- **Slice 1** (a18ffd984): the constructor, the tier, admission, precedence,
  and fixtures.
- **Slice 2:** `scripts/author-contracts.mjs build`/`check`, the specs, and
  the check wired into `make verify`.
- **Slice 3:** `author-contracts.mjs probe`, plus the rc.13 profile of the
  development collector.
- **Slice 4:** the pilot.
  - Five entries: three solid-query rc.0 cases (`useMutation`) and two router
    rc.4 cases (`useSearch`, `useParams`).
  - All three pairs pass on rc.13. The router pairs read in a component
    rendered through JSX: a route component's own body runs without a
    strict-read window, and rc.13 does not warn there.
  - Measured on the rc.13 corpus, after its lockfiles were made to name the
    rc.13 runtime they hold: violations 256 -> 266 (+10, -0). These are
    exactly the 10 ADR 0186 sites.
