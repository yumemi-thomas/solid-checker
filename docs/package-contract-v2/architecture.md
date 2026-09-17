# Package-contract architecture

## Outcome

The analyzer learns one interface: load an accepted contract for an exact
resolved import and query its normalized semantics. All compact-document,
summary, closure, artifact-case, proof-receipt, and schema-version complexity
stays behind that interface.

```text
package acquisition and host resolution
                 |
                 v
       Type Facts + compiler facts
                 |
                 v
       proposal generator (untrusted)
                 |
        +--------+---------+
        |                  |
        v                  v
 proof/fact sidecar    runtime probes
        |                  |
        +--------+---------+
                 v
          proof checker
                 |
          contract + receipt
                 |
                 v
   one decoder/normalizer/selector
                 |
                 v
        accepted semantic model
                 |
                 v
          Reactive IR consumers
```

## Source ownership bootstrap

The semantic architecture does not require separate repositories. Type Facts is
a deep module because of its process/session interface, not because its source
is remote. Its Go producer and Rust client are co-located in this repository so
producer, consumer, and proof changes land atomically:

```text
apps/solid-typefacts       Go producer and TypeScript-Go adapters
rust/crates/typefacts      Rust process/session client
rust/crates/solid-facts    normalized fact-domain integration
```

Solid 2 compiler source remains with the compiler's upstream owner. The checker
consumes an exact revision of `yumemi-thomas/solid`, based on
`solidjs/solid#next`, whose patch queue contains semantic-fact code only. The
fork may observe existing compiler decisions but may not change lowering,
generated output, diagnostics, runtime behavior, features, or performance.
Solid 1.x keeps its separate compiler fork.

These physical choices preserve the fact-domain seams below. Neither TypeScript-
Go objects nor Solid compiler AST nodes cross into Reactive IR.

## Modules and interfaces

### Contract acquisition module

Owned by `solid-facts-backend`.

Responsibilities:

- discover contract and receipt files;
- validate file and package identity;
- receive the host's actual resolved import;
- load only the selected artifact case;
- enforce resource limits;
- return an accepted normalized contract or a typed failure.

Its external interface is intentionally small:

```rust
fn load_accepted_contract(
    document: &[u8],
    receipt: &[u8],
    import: &ResolvedImport,
) -> Result<AcceptedContract, ContractFailure>;
```

The implementation may have internal seams for hashing, filesystem access, and
resolver adapters. Those seams do not become analyzer interfaces.

### Contract document module

Owned by `solid-facts-backend`. The only live main-document implementation is
`rust/crates/solid-facts-backend/src/contract_document.rs`. It reads and emits
stable schema version 1; there is no temporary-v2 or legacy-v1 compatibility
adapter. Phase 18 changed this one owner and every byte-bound artifact
atomically and gave the stable schema its final public path.

Responsibilities:

- parse private wire types;
- expand summary references without semantic loss;
- reject cycles, unused summaries, dangling references, and invalid closure;
- normalize guards, operations, values, identities, and artifact cases;
- compute canonical semantic and wire digests;
- validate the acceptance receipt;
- return the rich model owned by Reactive IR.

No downstream module sees summary IDs, `closed` arrays, schema spellings,
omission rules, aliases, or sidecar paths.

### Contract semantic model

Owned by `solid-reactive-ir`.

Responsibilities:

- represent open and closed claim domains;
- represent operation graphs, recursive values, resources, guards, ownership,
  and execution triggers;
- instantiate guarded behavior at a call site;
- join multiple possible guard cases monotonically;
- expose exact unresolved obligations;
- compose accepted dependency contracts;
- keep possibility distinct from guaranteed behavior.

Callers query semantic outcomes rather than fact tables or wire fields.

### Accepted analyzer query seam

`AcceptedContractIndex` is the Phase 12 analyzer-facing boundary. Acquisition
constructs it only from receipt-validated contracts and keys each contract by
the exact importer/specifier occurrence. An export query additionally requires
the complete runtime and declaration target identity; a matching public name
is not sufficient.

`CallSiteFacts` adapts the locally complete parts of the Type Facts invocation
transcript (selected signature, expanded argument count, and result protocol)
and exact demanded entity rows. Restricted guard atoms read only their own
leaf. An unresolved atom joins all still-possible cases and adds a typed local
reason; it does not open an unrelated claim. Artifact-case guards use the case
already selected from `ResolvedImport`, not a guessed call-site label.

Queries expose `KnowledgeSet` for possible behavior, cardinality-filtered
guaranteed operations, complete empty sets for proved absence, and `Unknown`
for no positive or negative proof. Native dialect knowledge wins when the
accepted contract is compatible; a proved contradiction is refused. The index
cache fingerprint binds its exact import mapping, package and manifest,
selected case, semantic/artifact/closure/proof/closed-claim roots, verifier
build, and proof policy in canonical sorted order.

### Proposal generator

Rust owns semantic inference and proposal construction. Node owns package
acquisition, process orchestration, temporary directories, runtime selection,
and probe workers.

The generator may emit positive candidates and closure proof obligations. It
cannot emit an accepted closed domain or acceptance receipt. Generator failure
opens the smallest affected claim domain; structural identity failure refuses
the exact artifact case.

### Proof checker

The proof checker shares normalized model and canonical hashing types, but no
generator inference implementation. It replays small proof rules over exact
Type Facts, compiler execution facts, artifact closure, accepted dependency
contracts, and probe falsification records.

Its interface is:

```rust
fn verify_proposal(
    proposal: ContractProposal,
    evidence: EvidenceBundle,
    policy: VerificationPolicy,
) -> Result<AcceptedBundle, VerificationFailure>;
```

`AcceptedBundle` contains the finalized compact document and acceptance receipt.

### Artifact resolver seam

Two adapters justify this seam:

- host/Type Facts resolution, which supplies the actual resolved path used by a
  configured project;
- standards-compatible package resolution used by standalone generation.

Both produce the same `ResolvedImport` model. Friendly host/mode/loader labels
may be reported for humans but never substitute for the exact resolution trace.

`solid-facts-backend::artifact_resolution` is the owning deep module. The exact
record contains logical and real package roots, manifest, independently selected
runtime and declaration artifacts, ordered branch traces, optional transform,
per-public-export runtime/declaration targets, canonical dependency closure,
and resolution authority. The authority chain is host, then Type Facts, then
standalone acquisition; only `Unattested` falls through. Ambiguity or structural
invalidity at a stronger source refuses the import.

Standalone acquisition follows ordered package `exports` objects, including
nested custom/default/import/require/browser/node/worker/deno/bun branches and
subpath-pattern precedence. Runtime and `types` resolution run independently.
The resolver records the selected JSON-pointer branch and every ordered step;
the contract compares exact branch provenance only after path and digest
identity have selected the actual artifact.

The canonical closure binds typed package-relative entries for runtime files,
declarations used for proof, manifests/resolution inputs, literal dynamic
chunks, and materialized generated output. External packages are edges to exact
artifact cases and accepted-contract digests. Paths, roles, bytes, transform
identity, dependency edges, and opaque hazards are length-delimited into one
SHA-256 identity. A nonliteral dynamic load, `eval`, native module, opaque WASM,
mutable unbound global, unmaterialized transform, or unaccepted external edge
opens only the named export/domain frontier. Anything outside the package,
missing from the closure, stale, multiply selected, or structurally
contradictory refuses the artifact case.

### Evidence store seam

Two adapters justify this seam:

- bundled read-only evidence/receipts shipped with the checker;
- project-local content-addressed cache used after local generation.

Ordinary analysis reads receipts through this seam but never raw evidence.

## Fact ownership

| Fact domain | Owns | Must not own |
| --- | --- | --- |
| Oxc syntax facts | Source structure, bindings, argument syntax, function nesting | Symbol identity, Solid runtime behavior |
| Type Facts | TypeScript identities, selected signatures, call binding, module resolution, finite type/value domains, semantic reference censuses | Solid tracking, ownership, scheduling |
| Solid compiler execution facts | Actual JSX/compiler lowering, tracked/eager/deferred/discarded execution, compiler-created owners, transformed server-function identities | Runtime-library callback semantics |
| Package contracts | External runtime operations, returned protocols, runtime-created resources, scheduling, cleanup, artifact cases | TypeScript syntax or compiler AST nodes |
| Reactive IR | Interprocedural composition, escape analysis, ownership propagation, guard instantiation, finding proof | Package-export resolution guesses |

Repository location is not fact ownership. In particular, co-located Type
Facts does not authorize Reactive IR to bypass its session interface, and a
checker-owned compiler branch does not authorize runtime-library facts or
compiler behavior patches.

## Failure locality

Structural failures refuse an artifact case:

- package integrity mismatch;
- artifact/declaration/closure mismatch;
- invalid resolution trace;
- zero or multiple selected cases;
- malformed operation graph;
- stale or invalid receipt.

Semantic incompleteness opens only its claim domain:

- unresolved callback escape;
- unbounded dynamic import affecting one operation;
- unknown recursive return leaf;
- unproved guard branch;
- opaque external dependency for one behavior.

An artifact case that asserts nothing about certifiable behavior is neither of
those. It is recorded in the proposal refusal sidecar's additive `inapplicable`
array — with a class and a reason, never certified, never counted as a refusal,
never suppressing a sibling case or the proposal — and omitted from the proposal
exactly as a refused case is. Three classes exist:

- `unpublished-conditional-target` — the runtime target is absent from the
  artifact and the selection traversed a private namespaced export condition;
- `non-module-target` — the runtime target's filename is one of an exact
  positive list of non-executable resources;
- `non-emitting-module-target` — the runtime target emits no JavaScript at all.
  Two premises answer that, and the member's suffix selects exactly one, which
  the recorded reason names: `erasable-statements`, where every module-level
  statement in the bytes is erasable and at least one declares a name (the
  filename is not read at all); and `declaration-file`, where the authenticated
  member's suffix is `.d.ts`/`.d.mts`/`.d.cts`, its bytes parse under
  declaration-file grammar, every statement is erasable or a re-export form, and
  an ambient gate finds no implementation body, initializer, expression
  statement or side-effect import anywhere. TypeScript decides declaration-file
  semantics by suffix and emits nothing for such a file at all, including for
  the re-export forms a plain module would emit — so the suffix is admitted as
  evidence, but only for a member the archive has authenticated and only
  conjoined with the ambient parse and gate.

Only the third is decided from file *content*, so only it carries an
applicability tag (`verifier-proved-type-only`) and travels to certification as
a declared claim in the planning request's `inapplicableCases`. Rust re-proves
the identical predicate against the authenticated archive's bytes
(`ArtifactSnapshot::prove_non_emitting_module_target`), re-deriving the premise
from the authenticated member path, and refuses the whole proposal when a claim
is refuted, naming the case and the first emitting statement. The other two are
properties of the export map and the artifact's member list, which the certifier
replays for every case anyway.

All three classes say "there is no runtime surface to certify here", not "a
consumer reaching this succeeds": a consumer importing the `.css` entrypoint, or
a `.d.ts` one, fails either way. The statement predicate has two
implementations — TypeScript's AST in the generator, Oxc's in the verifier — held
to one answer by the shared corpus `fixtures/module-emission/cases.json`.

The entire document is refused only when identity, selection, or normalization
cannot be trusted.

## Dependency direction

```text
wire decoder and receipt validator
              |
              v
normalized contract semantic model
              |
              v
Reactive IR and dialect consumers
```

JavaScript orchestration may call the Rust contract tool. It must not implement
a second semantic normalizer. Dialect modules may contribute native facts, but
shared infrastructure must not switch on individual Solid API names.

The build dependency direction is likewise one-way: the checker consumes the
local Type Facts interface and the pinned semantic trace; neither producer
imports Reactive IR or package-contract policy.

## Replacement test strategy

The interface is the test surface. Tests should load a document plus receipt
and assert semantic queries, selected operations, and unresolved obligations.
Once this surface is complete, tests that assert private expansion or duplicate
JavaScript normalization behavior should be deleted rather than layered beneath
the new interface.

## First-party RC.3 conformance seam

`solid-reactive-ir::contract_semantics::solid2_rc3` owns the internal Solid 2
RC.3 semantic corpus. It produces ordinary `ContractProposal` values and uses
only normalized concepts: operations, causal edges, resources, lifetimes,
ownership relations, cardinality, guards, recursive shapes, artifact cases,
and local stability. It does not know compact summaries, `closed` arrays,
aliases, schema versions, receipt JSON, or analyzer discovery.

The checked machine corpus under `benchmarks/package-contract-v2/phase13/`
binds those models to exact published file and closure identities and records
the proof/probe and six fixture classes required for every conformance row.
The replay helper verifies artifact bytes, the complete finite package-instance
census, exact TypeScript observations, and selected runtime traces. This is a
certification input, not a second normalizer: Rust remains the sole semantic
owner, and Phase 14 alone may connect the corpus to public producers, bundles,
and analysis discovery.

## Corpus and performance gate seam

The Phase 16 benchmark is outside ordinary analysis. The ecosystem runner owns
isolated exact-version installation and generator wall time. Rust owns the
receipt-issued 24-case measurements and builds them through the ordinary proof
checker before timing accepted loads and normalized export queries. A private
measured-bundle wrapper carries proposal, plan, proof, and timing scalars only
inside the benchmark path; `FirstPartyBundle` and `AcceptedContractIndex` do
not expose benchmark or raw-evidence state.

One artifact case that fails acquisition or normalized merge is recorded as an
exact refusal while independently mergeable cases remain in a partial proposal.
A structurally complete or partial proposal is still unaccepted. Corpus
coverage and runtime-probe non-observation cannot close any claim; only the
proof checker and matching receipt can create analyzer input.

The checked Phase 16 artifacts bind their source reports by digest and record
canonical and pretty main bytes, proposal-plan and raw proof-evidence bytes,
receipt bytes, bytes per export/operation, generation, proof verification,
accepted load, normalized query, and whole-process peak RSS. The source gate
also confirms the analyzer-facing index has no filesystem, process, network,
document-byte, receipt-byte, or sidecar field. Ordinary queries therefore stay
closed over normalized semantics after acquisition inputs are dropped.
