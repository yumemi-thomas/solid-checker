# Phase 7 completion report: artifact resolution and closure

Date: 2026-08-27

Branch: `codex/phase7-artifact-resolution-closure`

Authority: the frozen package-contract-v2 model, Node package-resolution
semantics, published `solid-js@2.0.0-rc.3` and related `@solidjs/*` package
artifacts, and the Phase 5 normalized semantic model

## Result

Phase 7 items 81-91 are implemented. Exact artifact resolution is now one deep
backend module, `solid-facts-backend::artifact_resolution`. It accepts an exact
host, Type Facts, or standalone resolution record, validates the complete
selection identity, selects exactly one normalized artifact case, replaces
provisional Phase 6 public-name bindings with independently resolved runtime
and declaration targets, and weakens only the precise claim domains exposed by
an opaque closure frontier.

The exit condition is enforced: selected semantics agree with the actual
resolved package, manifest, runtime artifact, declarations, transform,
dependency closure, entrypoint, runtime/types branches, and public-export
targets, or the case is refused. No missing resolver fact becomes negative
package behavior.

The loader still returns `AcceptanceUnavailable` after successful selection.
This is intentional. Phase 7 identifies the exact candidate semantics; only
Phase 11 proof replay and receipt verification may construct
`AcceptedContract`.

## Exact resolution model and authority

`ResolvedImport` records:

- specifier, importer, and requested package entrypoint;
- exact package name, version, integrity, logical root, and real root;
- manifest, runtime, declarations, and optional transform paths and SHA-256
  identities;
- independent ordered runtime and declaration resolution traces;
- exact runtime/declaration module and export name for every public export;
- canonical local closure, accepted dependency edges, and opaque hazards;
- the authority that produced the answer.

`ArtifactResolverChain` orders authority as host, Type Facts, then standalone
package resolution. Only `Unattested` falls through. An invalid or ambiguous
higher-authority answer refuses the import instead of being replaced by a more
convenient lower-authority answer. Duplicate exact rows remain ambiguous.

Ordinary Type Facts analysis already consumed compiler-resolved imports for
legacy contract binding. Phase 7 stops discarding the compiler-included path,
pre-realpath symlink spelling, exact resolver extension, owning package
version, and resolver-recorded package version. The WASM host interface carries
the same fields. This preserves the complete existing Type Facts resolution
answer for later consumer migration without prematurely moving the legacy
analyzer to schema v2.

## Standalone package resolution

`packages/cli/scripts/artifact-resolution.mjs` performs package acquisition for
standalone generation. It:

- finds the nearest nested `node_modules` installation while retaining logical
  and real roots;
- resolves exact and wildcard export subpaths with Node pattern precedence;
- traverses conditional objects in declaration order;
- supports nested custom, `default`, `import`, `require`, `browser`, `node`,
  `worker`, `deno`, and `bun` branches;
- rejects mixed subpath/condition maps, invalid or encoded traversal targets,
  blocked targets, missing targets, and zero-match condition sets;
- resolves runtime and declarations independently, with `types` active only on
  the declaration axis;
- records the selected JSON-pointer branch and ordered selection steps;
- resolves ESM direct exports, renamed reexports, imports forwarded as exports,
  and star exports, refusing ambiguous or cyclic export identity;
- retains literal dynamic chunks in the local closure.

The module is acquisition only. It is not wired into the legacy proposal
generator before Phase 8.

## Canonical closure and invariants

One closure manifest contains independently typed entries for runtime modules,
declarations used for proof, the package manifest, resolution inputs, literal
dynamic chunks, and generated/virtual output. A generated entry requires stable
bytes, a stable virtual identity, and an exact transform digest. Symlinked files
are hashed through their real path but retain their package-relative logical
identity; a symlink escaping the package root is refused.

External dependencies are not copied into a caller's local closure. They are
edges containing exact specifier, package name, artifact-case identity, and
accepted-contract digest. Contradictory duplicate files or dependency edges are
rejected.

The closure digest uses SHA-256 with the domain
`solid-checker:artifact-closure:v1`. Input order is erased by canonical sorting,
then typed, length-delimited values are hashed in this order:

1. file role, package-relative or stable virtual path, file-byte digest, and
   optional transform digest;
2. dependency specifier, package, artifact case, and accepted-contract digest;
3. hazard kind, exact source, affected exports, and affected claim domains.

Paths and roles are semantic. Identical bytes at a different path, the same
path with a different role, a different dependency edge, transform, generated
output, or opaque frontier produce a different closure digest. Rust and Node
tests pin the same empty-closure digest, so the two implementations cannot
silently drift to different framing.

Validation recomputes the digest, requires canonical order, lowercases SHA-256
hex, rejects traversal and malformed virtual identities, and rejects stale or
contradictory manifests. Runtime/declaration reexport targets must be the
selected root or an exact same-role member of the canonical closure.

## Local opaque frontiers

Closure hazards represent nonliteral dynamic loading, `eval`, native code,
opaque WASM, mutable unbound globals, unmaterialized transforms, and external
dependencies without an accepted contract. Each hazard names exact source,
affected exports, and affected immediate call domains.

Weakening is monotone and local:

- complete negative becomes unknown;
- complete positive becomes partial while retaining every known positive;
- partial and unknown remain unchanged;
- unrelated call domains, exports, operations, resources, and recursive value
  leaves remain unchanged.

This prevents an opaque edge from manufacturing negative proof while allowing
unrelated known semantics to remain usable.

## Focused and adversarial tests

Rust tests cover:

- canonical order equivalence and role-sensitive identity;
- the shared Node/Rust typed digest golden;
- identical bytes under different paths or dependency closures;
- generated-output byte and transform binding;
- internal symlink canonicalization and escaping-symlink refusal;
- zero-match, multiple-match, stale runtime, declaration, manifest, and closure
  identities;
- stronger-resolver refusal and authority fallthrough only on unattested;
- independent runtime/declaration renamed reexport binding;
- stale reexport target refusal;
- export- and domain-local hazard weakening with sibling knowledge preserved.

Node tests cover:

- nested/custom/default/import/require/browser/node/worker/deno/bun condition
  selection and ordered traces;
- independent runtime and declaration roots;
- subpath-pattern precedence, missing exports, invalid targets, mixed keys, and
  encoded traversal;
- independent renamed runtime/declaration reexports;
- accepted external dependency edges and literal dynamic chunks;
- nonliteral loading, `eval`, native, opaque WASM, and genuinely unbound mutable
  global hazards;
- same bytes with different local closure topology;
- nested symlinked installs and logical/real identity;
- materialized generated-output hashes.

## Producer, protocol, and generated artifacts

No Type Facts producer, Type Facts schema/protocol, compiler-facts producer,
Solid compiler fork, compiler pin, public package-contract schema, bundled
contract, generated contract, evidence sidecar, receipt, fixture snapshot, or
conformance artifact changed.

The Type Facts Rust consumer and WASM host input were extended only to retain
fields already present in the local Type Facts module-graph protocol. There is
no producer/client protocol split and no compiler change.

## Verification

Focused checks completed during implementation:

```text
bun packages/cli/node_modules/vitest/vitest.mjs run --config packages/cli/vitest.config.mjs packages/cli/test/artifact-resolution.test.mjs
  10 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts --lib
  61 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib
  168 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib
  46 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib artifact_resolution::tests
  7 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test contract_interface
  6 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-checker-wasm --lib
  passed (0 tests; compile gate)
bun run --cwd packages/cli test
  6 files, 56 tests passed; TypeScript declaration test passed
make verify
  passed in 50.20 s from the final source state; 94 fixture projects and 557
  findings matched, 289 ownership cases passed, and all workspace, dialect,
  TypeScript oracle, performance, contract, schema, formatting, and clippy
  gates passed without warnings
```

The green commit list and pull-request URL are added at handoff.

## Exact remaining open or uncertifiable cases

- The schema-v2 loader selects and binds semantics but cannot accept them until
  Phase 11 supplies proof replay and a fully bound receipt.
- The legacy proposal generator and package acquisition path are unchanged;
  Phase 8 owns their cutover to this resolver and Rust proposal interface.
- Existing bundled contracts and analyzer consumers remain on the legacy
  contract model until Phases 12-14. Phase 7 therefore changes no current
  finding or certification.
- Accepted external dependency edges carry an accepted-contract digest, but
  Phase 11 must still verify that referenced receipt before closure is
  accepted.
- Nonliteral dynamic loading, `eval`, native addons, opaque WASM, mutable
  unbound globals, missing stable transform bytes/identity, and unaccepted
  external dependencies remain open at their exact hazard frontier.
- CommonJS `module.exports` surfaces and loader-defined virtual module schemes
  do not receive guessed export identities. They are refused unless a stronger
  host supplies an exact record or later acquisition support proves them.
- Missing, ambiguous, cyclic, or out-of-closure reexports; invalid package
  targets; unresolved local modules; escaping symlinks; stale hashes; and zero
  or multiple artifact cases are structural refusals, not semantic negatives.
- Type Facts supplies exact compiler resolution provenance but not the complete
  runtime/declaration byte closure. Ordinary project analysis therefore cannot
  synthesize a replacement `ResolvedImport` from one Type Facts row alone; a
  host record or standalone acquisition must provide the remaining artifact
  identity until the consumer migration assembles both sources.

These boundaries preserve known unrelated facts and do not claim completeness
where the required artifact or acceptance premise remains absent.

## Handoff

- branch: `codex/phase7-artifact-resolution-closure`;
- implementation commit: `2bca6253` (`feat(contracts): implement artifact
  resolution closure`);
- solid-checker PR: <https://github.com/yumemi-thomas/solid-checker/pull/49>;
- this handoff metadata is the report-only follow-up commit on that PR;
- no upstream Solid PR was created.
