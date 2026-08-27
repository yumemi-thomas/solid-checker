# Phase 12 completion: analyzer integration

Date: 2026-08-28

## Outcome

Phase 12 is complete behind the private replacement boundary. Ordinary
analysis can obtain normalized semantics only by loading a temporary-v2
document together with a proof-issued receipt and the host's actual
`ResolvedImport`. Wire summaries, `closed` arrays, aliases, omission rules,
schema versions, and receipt JSON terminate inside
`solid-facts-backend::contract_interface`.

This is not the public schema cutover. Stable-schema discovery, generators,
probes, bundled contracts, process fixtures, CLI/WASM surfaces, and deletion of
the legacy decoder remain one atomic Phase 14 change.

## Accepted loading and invariants

`load_accepted_contract` now:

1. applies the bounded private schema-v2 decoder and normalizer;
2. checks the wire digest and receipt syntax/version;
3. selects and rebinds exactly one artifact case through the actual resolved
   package, manifest, entrypoint, traces, runtime, declarations, closure,
   transform, and per-export targets;
4. requires proof policy 1 and a non-empty verifier build;
5. recomputes and compares semantic, artifact, closure, and closed-claim roots;
6. preserves the proof root as opaque verifier authority; and
7. constructs `AcceptedContract` only after every available binding agrees.

Receipt replay recursively enumerates every closable finalized leaf: call
domains, tuple/object/choice membership, reactive/store capabilities,
operation owner productions, resource states/capabilities, and a guard
partition whose cases and selected-operation sets are all closed. Scalar
operation axes remain proof premises rather than closure claims, matching the
Phase 11 proposal/closure boundary.

An end-to-end backend test opens a proposed read domain, replays all eighteen
proof families, issues a receipt, loads the final compact document against an
exact import, and proves that only the resulting accepted index exposes the
export. Stale semantics, wire bytes, policy, artifact roots, closure roots, and
closed-claim roots are refused.

## Analyzer query model

`AcceptedContractIndex` is keyed by exact importer/specifier occurrence, so
nested installations cannot alias one another. Duplicate answers refuse the
import. Export lookup compares the full normalized `ExportIdentity`, including
entrypoint, public spelling, runtime module/export, and declaration
module/export; a matching name alone is not accepted.

`CallSiteFacts` is the single adapter into restricted guard evaluation. It
reads selected signature, exact expanded argument count, and finite result
protocol from `InvocationTranscript`; exact demanded entity rows supply local
constant and runtime-kind facts. Literal candidates remain partial because the
Type Facts producer explicitly does not call that bounded list exhaustive.
Runtime `other` expands conservatively to plain, Promise, and AsyncIterable
possibilities rather than being mislabeled as plain. Exact AST/Type Facts
property and tuple answers can be supplied at their own argument/path leaf.

All guard axes remain independent: signature, arity, literal, value kind,
property presence/callability, tuple alternative, result protocol, and artifact
case. Artifact-case atoms use the case already selected by the resolver. An
unresolved guard selects the monotone union of possible cases, reports a typed
`OpenDomainDiagnostic`, and cannot create guaranteed behavior. Operation-local
guards are also applied to operation and callback queries.

Each claim returns the four-state `KnowledgeSet` unchanged:

- `Unknown`: no positive or negative proof;
- `Partial(items)`: known possible positives with an open remainder;
- `Complete(items)`: the exhaustive positive set; and
- `Complete([])`: proved absence.

Possible operations are every still-reachable positive. Guaranteed operations
add the independent cardinality requirement and are withheld when guard
selection is unresolved. An open writes/throws/callback leaf does not weaken a
closed reads leaf or a recursive sibling.

Native dialect knowledge wins when it is compatible with accepted contract
knowledge. A contradiction proved by either closed side returns
`NativeContractConflict`; an open side cannot manufacture a disagreement or a
negative.

## Cache identity

The accepted index derives one deterministic, order-independent cache
fingerprint over:

- exact importer and specifier;
- package name, version, integrity, and manifest artifact;
- selected artifact case;
- receipt and semantic-model versions;
- semantic, artifact, dependency-closure, proof, and closed-claim roots; and
- verifier build and proof policy.

Changing acquisition order is equivalent. Changing an import binding, package
or artifact identity, semantic root, proof/closure root, verifier build, or
policy produces a different key.

## Diagnostics and TypeScript boundary

Open-domain results carry structured local reasons: the exact `ClaimPath`, an
unresolved guard partition, or the exact operation whose own guard is
unresolved. Consumers need not decode schema mechanics to explain refusal.

No finding kind or finding output changed in this phase. The existing
real-typings oracle has `tsc`-silent witnesses for every finding kind currently
driven by a package contract: `strict-read-untracked` and
`reactive-dispatch-unresolved` in both dialects, `missing-owner`, `prefer-for`,
and `no-destructure`. `package-contract-incomplete` remains an explicit oracle
exemption because it reports the authority/presence of an external contract
artifact, which no TypeScript expression can encode. The complete oracle is
rerun by `make verify`.

## Tests and verification

Focused tests added or expanded cover:

- stored receipt replay and mutation of semantic/artifact/closure/closed roots;
- refusal of a forged acceptance with no locally closed claim;
- receipt-policy drift;
- proof-issued receipt to exact accepted analyzer index;
- exact import and export identity plus duplicate refusal;
- deterministic cache identity and receipt-policy invalidation;
- every restricted guard axis and exact invocation-transcript adaptation;
- monotone unresolved-guard joins with no invented guarantee;
- possible versus guaranteed cardinality;
- complete absence versus unknown;
- native precedence and contradiction refusal;
- exact artifact-case guard selection;
- local Type Facts leaf uncertainty and sibling isolation; and
- typed local open-domain diagnostics.

Commands completed while iterating:

```text
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib contract_semantics
  49 passed
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib contract_interface::tests
  1 passed
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test contract_interface
  8 passed
cargo +1.97 clippy --manifest-path rust/Cargo.toml -p solid-reactive-ir -p solid-facts-backend --all-targets -- -D warnings
  passed
```

Final handoff verification:

```text
make verify
  passed in 43.37s
  workspace tests, v1/v2 backend and WASM feature checks, 94-project coverage
  (557 findings), 289 ownership cases, TypeScript oracle, contract conformance,
  CLI tests, and performance certification all passed
```

The first sandboxed invocation reached the Type Facts crash/restart process
test but the sandbox denied Go's module stat-cache write. The identical
unrestricted command passed, including that previously interrupted test.

## Producer and generated-artifact impact

Type Facts producer/protocol, the Rust Type Facts client, and Solid compiler
facts did not change. No compiler fork or compiler pin moved. No bundled
contract, public schema, receipt bundle, snapshot, dialect manifest, runtime
lock, or other generated artifact changed.

## Exact remaining open or uncertifiable cases

- Phase 13 still owns encoding and proving every published Solid
  `2.0.0-rc.3` conformance row. A missing row or transcript stays locally open.
- Phase 14 still owns public contract/receipt discovery and the atomic producer,
  probe, verifier/tooling, backend-fixture, CLI/WASM, bundled-contract, and
  legacy-decoder migration. The current stable-schema product path is therefore
  unchanged by this private integration.
- Missing or incomplete signature, spread/binding, literal, property,
  callability, tuple-alternative, result-protocol, or artifact facts leave only
  the affected guard unresolved.
- An open claim domain, recursive leaf, operation guard, or guard remainder is
  uncertifiable locally; unrelated closed claims remain usable.
- Receipt absence/mismatch, proof-policy drift, ambiguous imports/exports,
  native-contract contradictions, artifact-selection conflicts, and opaque
  closure hazards refuse their exact boundary. None is converted into negative
  behavior.
