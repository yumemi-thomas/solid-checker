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

Owned by `solid-facts-backend`, beginning at
`rust/crates/solid-facts-backend/src/contract_document.rs`.

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
