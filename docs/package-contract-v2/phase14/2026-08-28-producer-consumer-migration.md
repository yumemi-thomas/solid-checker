# Phase 14 completion report — producer and consumer migration

Date: 2026-08-28
Branch: `codex/phase14-producer-consumer-migration`

## Outcome

Every live package-contract producer and consumer now speaks temporary main
`schemaVersion: 2`. Package proposals, missing-package sweeps, first-party
bundles, runtime probes, review, proof verification, native discovery, WASM,
fixtures, differential/conformance gates, and dialect manifests use the same
normalized workflow. The schema-1 public decoder and generator were deleted;
there is no compatibility fallback.

Ordinary analysis accepts one value only after all three inputs agree:

1. exact normalized temporary-v2 document bytes;
2. a proof-issued receipt bound to those bytes and semantics;
3. an independently acquired exact `ResolvedImport` for one importer and
   specifier.

The backend validates and normalizes that envelope once. The reactive analyzer
receives only an `AcceptedContractIndex`; schema versions, summary IDs, aliases,
omission rules, receipts, catalogs, and artifact-resolution wire records do not
cross the normalization boundary.

## Model and invariants carried through the migration

The internal model retains all four local claim-knowledge states:

- `Unknown`: no usable premise;
- `Partial`: known members without an exhaustive census;
- `CompletePositive`: a proved exhaustive non-empty set;
- `CompleteNegative`: a proved exhaustive empty set.

Recursive tuple, object, choice, Promise, AsyncIterable, resource, and returned
value leaves carry their own knowledge. Recursive traversal reports the exact
open leaf; it does not widen to the parent or contaminate a sibling. Missing
evidence therefore never becomes a negative claim.

Operations are exact nodes joined by data, control, error, cleanup, ordering,
and lifetime edges. Trigger, event/schedule, tracking, ownership, and
possible-versus-guaranteed cardinality remain independent axes. Owner
requirements, owner production, and owner kind remain separate. Operations
create/use/dispose typed resources; resource state and lifetime must agree with
operation reachability and causal edges. Restricted guards form finite
non-overlapping partitions. Unresolved selection uses a monotone join that can
add possible behavior but cannot create guaranteed behavior or negative proof.

Artifact cases bind exact package, entrypoint, runtime, declaration, closure,
conditional-resolution, and export identity. Experimental status remains local
to one case/export. Validation still rejects dangling references, invalid or
cyclic graphs, contradictory capabilities/ownership/resource states,
overlapping or uncovered closed guards, invalid cardinality, orphan claim IDs,
and proposed false closure.

## Producers and tooling

- `generate-package-contract-v2.mjs` acquires exact artifacts and finite
  condition partitions; Rust owns inference, normalization, merging, compact
  encoding, and proposal-plan construction.
- Missing-contract generation uses the same producer and requires exact
  package-manager integrity. It leaves existing proposals untouched and
  refuses linked/local packages without registry identity.
- External export-all boundaries without independently accepted semantics are
  refused. A generated dependency proposal is never accepted as proof for its
  parent.
- Review expands normalized artifact cases and recursively open claims without
  mutating closure.
- Probe plan, driver, worker, and harness use claim-addressed temporary-v2
  sessions. Rust authorizes and classifies semantic events; Node only manages
  fresh processes and files. Probes can witness positives or contradict one
  proposed closure leaf, never establish negative or accepted closure.
- Proof verification replays every required family, applies contradictions,
  closes only named leaves, renormalizes, and is the only receipt issuer.
- Closure, pin, differential, corpus, bundle, review, and obligation scripts no
  longer contain a second JavaScript semantic normalizer.

The native generator-local accumulator is deliberately not a wire type. Its
open/known values normalize immediately into the semantic model. Serialized
unknown sentinels, public variants, entrypoint condition arrays, inline
evidence, generator trust tiers, name-only dependency selection, and same-run
dependency trust were removed. The old `solid-contract-gen`, legacy decoder,
legacy native builder inputs, and legacy JavaScript document helpers/tests were
deleted.

## Consumers

Native discovery reads `.solid-checker/accepted-contracts.json`. Each catalog
row supplies document and receipt paths plus the full exact import resolution.
Duplicate bindings, missing files, invalid receipts, stale bytes, mismatched
artifact identity, ambiguous selection, and unresolved exports fail closed.

WASM exposes the same boundary as `acceptedContracts`. Main document and
receipt are exact JSON strings, not parsed values, because the receipt binds
wire bytes. The host also supplies the complete Phase 7 resolution record. An
omitted contract never falls back to package-name matching.

The daemon cache hashes catalog contents and every referenced document/receipt,
not just a path. Analyzer construction and incremental builders no longer
accept legacy `PackageContract` inputs.

## Canonical identity and receipts

Canonical semantic digests include semantic-model version, exact package and
artifact identity, export identity, all four knowledge states, recursive leaf
paths, normalized operations/edges/resources/guards, and local experimental
state. Unordered sets and maps are sorted; equivalent numeric and restricted-
guard spellings normalize identically; wire summary names, aliases, formatting,
omission choices, and evidence ordering do not affect semantic identity.

Receipts additionally bind exact main-document bytes, selected artifact and
closure roots, proof-policy and verifier identity, proof inputs, and the exact
closed-claim root. Any wire, semantic, artifact, closure, proof-policy, or
closed-claim drift invalidates replay.

Because `wireDigest` binds raw bytes rather than parsed JSON, `.gitattributes`
forces LF checkout bytes for JSON on every platform. A Windows checkout may
not translate an accepted document to CRLF while retaining a receipt issued
for its LF bytes.

## First-party and fixture artifacts

`make contracts` regenerated 24 receipt-issued artifact cases in both
`pkg/contracts/bundled/` and
`rust/crates/solid-dialect/contracts/`, together with bundle indexes. The
runtime lock and both dialect manifests now enumerate the temporary-v2 bundle
shape.

Solid 1 covers exact `solid-js@1.9.14`, scheduled 1.5.3, debounce 1.3.0, and
rootless 1.5.4 cases. Solid 2 covers exact RC.3 `solid-js`, `@solidjs/web`, and
`@solidjs/signals` cases. The signals bundle also preserves the exact RC.3
`isEqual` plain-return semantics through a reviewed support record bound to the
published runtime body, declaration, artifact closure, and export identity;
unreviewed sibling exports do not inherit that knowledge. Package closure
censuses use an explicit ASCII-folded path order with an original-path
tie-break, reproducing the already recorded Phase 13 closure roots without
depending on the host locale. Symlinked installs are accepted only when the
lexical package-manager install canonicalizes to the resolver's exact package
root, so its lock integrity can be recovered without package-name guessing.

All backend/process contract fixtures now carry temporary-v2 documents,
receipts, and exact accepted catalogs. The generator corpus contains 39
fixtures: 27 emitted artifact cases and 13 explicit fail-closed refusals, with
53 possible operations, 178 proof candidates, and 989 local open claims.

Ten findings snapshots changed after normalized comparison:

- reduced or ambient fixture artifacts no longer borrow published first-party
  semantics by package name;
- stale missing-export noise disappeared where the exact accepted artifact has
  complete export knowledge;
- obsolete Solid 1/2 legacy bundle-gap rows disappeared;
- the package-contract remedy now describes proposal/proof/receipt/catalog
  acceptance;
- comment-only fixture clarifications moved three recorded byte offsets.

No snapshot was changed to turn missing evidence into a violation or negative
proof.

## Tests and verification

Focused/property coverage added or retained by the migration includes:

- wire-order and summary-name normalization equivalence;
- deterministic semantic, proof, receipt, plan, transcript, and cache digests;
- four-state knowledge identity and recursive sibling locality;
- monotone unresolved guard joins;
- invalid graph, resource, cardinality, capability, ownership, and guard
  rejection;
- false closure, stale receipt, wrong artifact/export identity, and duplicate
  exact import refusal;
- accepted-only native/WASM process paths and explicit schema-1 refusal;
- source-versus-receipt-issued consumer differential equality;
- repository-root Bun loading of the TypeScript compiler API through its
  standards-compatible ESM default export;
- standalone corpus installation and path-filter coverage for the complete
  temporary-v2 producer script surface;
- preservation of the exact TypeScript runtime dependency in assembled npm
  packages;
- LF checkout enforcement for receipt-bound bundle, dialect, and fixture
  documents.

Commands run during implementation:

| Command | Result |
| --- | --- |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib` | 188 passed |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib` | 80 passed |
| armed `contracts_process`, `diagnostics_process`, and `dialects_process` | 5 + 15 + 37 passed |
| `bun run --cwd packages/cli test` | 4 files, 39 tests passed; TypeScript check passed |
| `bun run --cwd packages/wasm test` | 2 files, 6 tests passed |
| temporary-v2 contract corpus, update then compare | 39 fixtures checked; 13 refusals; 27 cases; 53 operations; 178 proof candidates; 989 open claims |
| contract differential | source and receipt-issued consumers agree; 0 findings |
| coverage compare after intentional update | 94 projects, 542 findings |
| armed `bun scripts/tsc-oracle-gate.mjs` | 161 rule cases and 41 keystones passed |
| `make contract-conformance` | 24 cases and both physical locations reproducible; 7 live npm pins verified; composed bundles passed |
| `make verify` | passed in 223.03 seconds for the migration and 139.57 seconds after the final CI portability fix |

The initial contract-conformance invocation was sandboxed from DNS and failed
only its seven live registry lookups. The authorized rerun performed all seven
lookups and passed. The final `make verify` also passed formatting, Go checks,
dependency installation, Clippy, both backend/WASM feature configurations, the
compiler-identity and Type Facts stamp gates, all workspace tests, coverage,
the TypeScript oracle, ownership (289 cases and 465 ledger rows, none pending),
performance, CLI/WASM tests, obligation audit (7 obligations and 11 closures),
all 24 bundled cases in both locations, all seven live pins, and composed
contract conformance.

The first PR run exposed two platform-only assumptions that local macOS did
not exercise. The standalone Linux corpus workflow did not install the CLI
package at all after Phase 14 made TypeScript part of the live producer; it now
performs the frozen install and watches the full producer script tree instead
of the deleted schema-1 generator path. TypeScript 5.9.3 is an exact runtime
dependency, package assembly preserves it, and artifact resolution uses and
validates its standards-compatible ESM default API. Windows Git translated
accepted JSON to CRLF, so the embedded document no longer matched its receipt's
LF `wireDigest`; the LF checkout policy above prevents that byte drift. The
original 39-fixture contract-corpus command, dependency-lock checks, package
assembly tests, and focused regressions passed before the final full
verification run.

## Type Facts, compiler facts, and generated binaries

No Type Facts producer/client/schema/protocol or Solid compiler semantic-fact
code changed. No compiler pin, Cargo dependency pin, compiler identity file, or
checked Type Facts binary changed. The Phase 14 abstraction was completed from
existing facts.

Generated source-controlled artifacts changed: temporary-v2 main documents,
receipts, bundle indexes, runtime lock, dialect manifests, generator corpus
snapshots/plans/refusals, accepted fixture catalogs, WASM types, and the ten
reviewed findings snapshots. Build outputs under `rust/target` were used only
for verification and are not source artifacts.

## Exact remaining open or uncertifiable cases

- Proposals remain unaccepted until every selected closed claim passes all
  proof families and receipt issuance. Passing probes are not proof.
- Wildcard/unbounded export maps, contradictory condition selections,
  unsupported class/namespace surfaces, non-literal dynamic imports,
  unresolvable callable kind, and external export-all without independent
  accepted semantics are generator refusals.
- Linked/local packages without exact registry integrity; missing, ambiguous,
  or byte-different runtime/declaration/export identity; closure hazards; stale
  or missing receipts; and unresolved import bindings remain uncertifiable.
- Recursive value leaves and operation axes stay locally open when their exact
  premise is missing; unrelated known siblings remain usable.
- Solid 1 `jsx-runtime` and `jsx-dev-runtime` subpaths with no common
  runtime/declaration value binding remain census-only, without an accepted
  semantic case.
- Phase 13 RC.3 open domains remain open: the server-functions client
  declaration's TypeScript-owned self-error; real-browser DOM, delegation, and
  hydration observation; request-context/transport integration; user
  serialization; dynamic payload/target/selection leaves; and unstable frames
  protocol details.
- For `@solidjs/signals`, only the reviewed `isEqual` support claim and the
  separately conformance-proved exports are accepted; other exports and exact
  leaves remain open rather than borrowing sibling semantics.
- Reduced first-party fixture artifacts intentionally refuse the published
  bundle when exact bytes and closure do not match.

Phase 14 does not claim complete package coverage. It completes the atomic
producer/consumer migration while preserving exact fail-closed domains.

## Handoff

- Branch: `codex/phase14-producer-consumer-migration`
- Implementation commit: `474c101f` (`feat: migrate package contracts to
  normalized v2`)
- Pull request: <https://github.com/yumemi-thomas/solid-checker/pull/56>

The handoff metadata itself is a documentation-only follow-up commit. The full
`make verify` result above was recorded against the implementation commit; the
follow-up changes no executable or generated artifact.
