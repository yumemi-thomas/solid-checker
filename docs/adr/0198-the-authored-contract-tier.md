# ADR 0198: The authored contract tier

- Status: accepted (2026-10-05); implemented in slices, see *Implementation
  status*. Track B of `docs/2026-10-05-package-direction.md`, implementing
  ADR 0189. Superseded in part by [ADR 0228](0228-the-certified-contract-tier-is-retired.md) (2026-10-08): the certified-tier
  precedence and bootstrap (decisions 4 and 6). The authored tier stays in
  force.
- Owners:
  - `accept_authored` in `solid-reactive-ir/src/contract_semantics/proof.rs`;
  - `solid-facts-backend/src/authored_contracts.rs` (the tier, its admission
    and its index);
  - `pkg/contracts/authored/` (documents and index);
  - the per-claim probe gate (slice 3).
- Relation: the first implementation of ADR 0189. Project catalogs keep their
  own rules. The certified tier (ADR 0186) was retired by ADR 0228.

## Context

Historical context for the pilot: the certified tier described here was
retired by ADR 0228.

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
4. **Precedence:** project catalogs, then the authored tier. There is no
   certified fallback since ADR 0228. An authored entry is keyed
   `authored:…`, so it never collides with a project catalog's acceptance.
5. **Every claim has a probe case** (slice 3): a misuse/correct pair run in
   headless Chrome on the published package at the listed version and the
   stated Solid runtime, in the misuse ledger's harness. The misuse case must
   raise the dev diagnostic the claim implies (`STRICT_READ_UNTRACKED` for an
   untracked read of a returned accessor or store), and the correct case must
   not. A claim without a passing pair is not shipped.
6. **Documents start from the version's own identity.** Every spec names an
   `identity.json` (ADR 0207) holding the version's artifact cases: identity,
   case structure, resolution, declaration targets and file digests. (The
   pilot took them from the certified bundle for that version; ADR 0228
   moved every spec to its own file.) A spec with none is an error. Every export's
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
     reproduce the identity's snapshot root and artifact digests, or whose Solid runtime,
     resolved from the package's own directory, is another release. Results go
     to `pkg/contracts/authored/probe-results.json`.
   - `build` writes the documents, the index and `embedded.rs`. A claim ships
     only with a `passed` result on the spec's runtime. A document's case
     carries only identity fields (artifact, declarations, resolution); any
     other case field is refused rather than carried unprobed. Summary ids are
     readable (`authored-<export>`, `open-<shape>`), since decoding does not
     require content-addressed ids for a one-case document.
   - Only browser cases are authored (export conditions that include
     `browser`). The probes run in Chrome, so a claim is evidence for the
     browser host and nothing else.
   - Specs of several versions may share one directory of pairs (`"pairs"` in
     `spec.json`; a `specs/_…` directory is not a spec). Each version is still
     probed in its own install.
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
- **`@solidjs/router`:** `returns` claims for seven hooks.
  - Claims: `useLocation` and `useParams` return a store (getters and proxies
    over memos); `useSearchParams` returns a tuple whose first item is a store
    (the setter is not claimed); `useIsRouting`, `useMatch`, `useHref` and
    `useResolvedPath` return accessors.
  - Probed in every installed version that runs on rc.13:
    - next.16, next.17, next.18 and next.26 pass all seven pairs;
    - next.19 to next.24 cannot load on rc.13 (they import
      `parseServerFunctionUrl`, which rc.13's `@solidjs/web` no longer
      exports), so they get no claims.
  - The checker flags each misuse twin and leaves each correct twin clean.
  - Measured (corpus hidden npm lockfiles relocked as well): violations
    256 -> 267 (+11, -0), the ten TanStack sites plus one router site, in
    `donegeon-client` (`createSignal(…(location.search))` in a route
    component body). A runtime twin of that shape on the app's install raises
    `STRICT_READ_UNTRACKED` at the read: unlike TanStack's, this router
    renders route components with a strict-read window.
  - Not admitted where the installed integrity is not a stated fact:
    `beacon-web` (binary `bun.lockb`, and a `package-lock.json` without
    integrities) and `error-menu-web` (two Solid versions in one lockfile).
- **`@tanstack/solid-query` options callbacks:** `useQuery` and
  `useInfiniteQuery`, on 6.0.0-rc.0 and rc.3.
  - Claim: argument 0 is invoked on every call, on the same stack, as the
    tracked compute of an owner the hook creates. Both versions call it
    inside `createMemo(() => options())`. Owner requirements stay unclaimed.
  - Each misuse twin writes a signal inside the options function and raises
    `REACTIVE_WRITE_IN_OWNED_SCOPE`; each correct twin reads one and raises
    nothing.
  - A created owner needs a named resource and a positive ownership-production
    claim. A document that does not decode makes every project fail at load,
    and `author-contracts.mjs check` does not catch that: only the
    `the_shipped_index_loads` unit test does. Rebuild and run a project after
    every tier change.
  - Measured on the rc.13 corpus: 33 `SC1001` callback reads leave
    uncertifiable; violations unchanged at 267.
  - Not cleared: `ai-memory-ui`'s own `useQuery` wrapper (46 sites) invokes
    the options inside the literal it hands to the real hook. Carrying the
    tracked role through a project helper is A3's helper step.
- **`@tanstack/solid-query` returns (2026-10-06):** `useQuery` on 6.0.0-rc.0
  and rc.3, and `useMutation` on rc.3.
  - Claim: the hook returns a store. rc.3's `useQuery` result is an object of
    getters over the query's projections and memos (`data`, `status`,
    `isPending`, …), and its `useMutation` result reads signals the same way.
  - One export may now state more than one claim. A spec names each further
    probe pair in the export's `probes` (`{ label, rule, why }`, exercised by
    `<export>.<label>.misuse.tsx` and `.correct.tsx`), and the export's
    `call` ships only when every pair passed. `useQuery` keeps its callbacks
    pair and adds `useQuery.returns`.
  - Each misuse twin reads `query.status` (or `mutation.status`) in the
    component body and raises `STRICT_READ_UNTRACKED`. Each correct twin reads
    it in JSX and raises nothing. All pairs pass on both versions.
  - The checker flags the misuse twin (`SC1001`) and leaves the correct twin
    clean.
  - The import notice still lists `returns` as open. A `store` return gives a
    property read its meaning; it does not close the domain (the value's
    shape and async behaviour).
  - Measured on the rc.13 corpus: nothing moved (violations 285,
    uncertifiable 3,414). No admitted app reads a query or mutation result
    untracked in a component body.
