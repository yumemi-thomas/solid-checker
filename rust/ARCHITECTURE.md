# Workspace architecture

The workspace separates what is true of *any* Solid checker from what is true
of *one Solid version*. Infrastructure lives under `crates/`; everything
specific to a Solid dialect — its rule catalog and its JSX compiler — lives
under `dialects/`.

```
rust/
├── crates/                      # dialect-independent infrastructure
│   ├── solid-facts/             # the fact model
│   │   ├── core                 # spans, source paths/hashes, generations
│   │   ├── ast                  # Oxc syntax facts
│   │   ├── compiler             # ExecutionMap + CompilerFactsProvider seam
│   │   └── (root)               # per-file/per-project joins with Type Facts
│   ├── solid-dialect/           # the vocabulary both versions answer through
│   ├── solid-reactive-ir/       # reactive program IR + the Finding model
│   ├── solid-facts-backend/     # daemon, caches, snapshots, contracts, CLI
│   │   └── wire.rs              # sources, edits, and Type Facts provider interface
│   └── solid-checker-wasm/      # process-free Node-API/WASI entry point
└── dialects/
    ├── solid-v1/                # everything Solid 1.x-specific
    │   ├── dialect.json         # contract/rule assembly manifest
    │   ├── rules/               # solid-v1-rules: the 1.x catalog
    │   └── compiler/            # solid-v1-compiler: the 1.x compiler adapter
    └── solid-v2/                # everything Solid 2.0-specific
        ├── dialect.json         # contract/rule assembly manifest
        ├── rules/               # solid-v2-rules: rule catalog + solve()
        └── compiler/            # solid-v2-compiler: Solid compiler adapter
```

## Version ownership at a glance

“Shared” means the algorithm is common, not that the two runtimes behave the
same. Shared code asks the selected vocabulary and receives the 1.x or 2.0
answer; it must not switch on an API spelling itself.

| Concern | Shared module / seam | Solid 1.x ownership | Solid 2.0 ownership |
| --- | --- | --- | --- |
| Syntax, TypeScript facts, reactive IR, caches | `crates/solid-facts`, `crates/solid-reactive-ir`, backend infrastructure | No separate implementation | No separate implementation |
| Primitive names, callback semantics, ownership, boundaries, import modules | `crates/solid-dialect::Dialect`; consumers read one `CallbackSemantics` descriptor per call argument | `solid-dialect/src/solid_1x.rs` (`Solid1x`, `Version::V1`) | `solid-dialect/src/solid_2.rs` (`Solid2`, `Version::V2`) |
| JSX compiler facts | `CompilerFactsProvider` | `dialects/solid-v1/compiler` | `dialects/solid-v2/compiler` |
| Rule projection and wording | `solid-reactive-ir::projection` owns typed finding seeds, shared selection, and finding assembly | `dialects/solid-v1/rules` declares capabilities and maps every supported seed to 1.x identity, severity, message, hint, and evidence | `dialects/solid-v2/rules` declares capabilities and maps every supported seed to 2.0 identity, severity, message, hint, and evidence |
| ESLint-era file-local checks | fact helpers live in `solid-reactive-ir::upstream_compat` | `solid1x_*` modules; executed only for `Version::V1` | Not executed |
| Shared static and fine-grained defects | `StaticDefectKind`, populated by static analysis and `upstream_compat::shared_reactivity`; contains no rule prose | Projected and worded by the 1.x catalog | Projected and worded by the 2.0 catalog; the async-tracked-scope check is omitted |
| Package contracts | temporary-v2 wire decode, receipt validation, and exact artifact acquisition terminate in `solid-facts-backend`; the wire-independent model and accepted semantic index live in `solid-reactive-ir::contract_semantics`; every bundled package is declared in the dialect manifest | receipt-issued exact `solid-js@1.9.14` plus scheduled, debounce, and rootless artifact cases | receipt-issued exact `solid-js@2.0.0-rc.3`, `@solidjs/web@2.0.0-rc.3`, and `@solidjs/signals@2.0.0-rc.3` artifact cases |

At runtime the stable dialect ids are `solid-v1` and `solid-v2`. In Rust,
`Version::V1` always means Solid 1.x and `Version::V2` always means Solid 2.0;
other protocol/schema versions are unrelated.

The same stable ids also prefix contract-generator targets
(`solid-v1/solid-js`, `solid-v2/solidjs-web`), contract artifact directories,
evidence labels, and shipped rule-manifest filenames. ESLint's `v1` and `v2`
flat-config keys remain intentionally short compatibility names stored as data
inside those manifests; they are not alternate dialect identities.

## The three dialect seams

**Vocabulary.** `solid-dialect` owns everything version-specific about
Solid's *vocabulary* — which names are primitives, which argument of a call
is its callback, which JSX tags open a boundary. The reactive engine takes a
`&dyn solid_dialect::Dialect` and asks; it does not know. This is the seam
ADR 0006 reopened so one engine could serve two dialects rather than one
engine per branch.

**Compiler.** `solid_facts::compiler::CompilerFactsProvider` is the checker's
whole view of a Solid JSX compiler: `AnalysisRequest` in, validated
`ExecutionMap` out. `solid-v2-compiler` implements it over the pinned
`solidjs-compiler` semantic trace, and `solid-v1-compiler` over the
same crate name from the pinned `solid-1x-compiler` fork; no other crate
speaks a compiler's own types. The analysis pipeline in `solid-facts-backend`
is generic over the trait. Callers obtain the selected adapter from the
composition bundle's compiler factory; the backend root does not re-export a
preferred dialect's concrete compiler. Their traces report execution sites
totally and compiler-established ownership regions conservatively. Component
identity composes compiler JSX call/use facts, TypeFacts aliases, AST return
shape, and an explicit 1.x compatibility convention; runtime callback
ownership remains contract-driven.

**Rules.** `solid-reactive-ir` builds a `Program` and defines the
dialect-neutral diagnostic model (`Finding`, `EvidenceStep`, `RuleMetadata`).
Its `projection` module is the single translation seam: it selects reportable
rows, turns them into a closed `FindingSeed` vocabulary, and assembles the
final `Finding`. Each rules crate is a `CatalogWording` adapter that declares
which optional tables it supports and exhaustively maps supported seeds to
rule identity, severity, message, hint, and evidence. `solid-v2-rules` owns
the 26-rule Solid 2.0 catalog; `solid-v1-rules` owns the 18-rule 1.x catalog
(`v1/<rule>` names, spanning the engine slices under 1.x vocabulary plus the
eslint-plugin-solid file-local surface). The wording duplication between
them is deliberate: a 1.x diagnostic never tells its reader to call an API
their Solid version does not have.

**Composition.** `solid_facts_backend::dialect::Dialect` bundles everything a
Solid version contributes: the compiler-provider factory, the catalog's
`solve`, rule documentation, package-contract finding projection, and the
bundled contract set, plus a stable `id`. Dialects register in
`dialect::ALL` and are resolved at the entry points — the CLI's `--dialect`
flag and the wasm request's optional `dialect` field, defaulting to detection
from the project's resolved `solid-js` version with `solid-v2` as the
fallback when nothing resolves — then threaded as a value through the whole
pipeline: build
functions, retained sessions, diagnostics, and the daemon. No backend code
names a dialect crate outside the registry. The dialect `id` is folded into
the compiler cache key, the retained diagnostic identity, and the daemon
socket identity, so artifacts from two dialects can never answer for each
other. `alternate_dialect_flows_through_native_pipeline` in the backend's
tests proves a non-default dialect's compiler and catalog flow end to end.
Semantic-demand planning follows the same path: imports are resolved through
the selected vocabulary's owned modules, export tables, primitives, and
reactive-source classification rather than backend-owned API spellings.

**Assembly manifest.** Each `rust/dialects/solid-vN/dialect.json` records the
shipped rule manifest and every contract artifact contributed by that dialect.
`scripts/dialect-manifests.mjs` validates and enumerates it for Makefile
generation/check targets and composed-contract drift checks;
`check-bundled-contracts.mjs` derives its runtime-probe set from the same data.
The ESLint adapter independently enumerates the resulting
`rules-solid-vN.json` artifacts and reads their `dialect`, `config`, and
`namespace` fields. See `docs/adding-a-dialect.md` for the forward checklist.

**Payload features.** `solid-facts-backend` and `solid-checker-wasm` expose
`dialect-v1` and `dialect-v2`, with both enabled by default. Each feature owns
its registry entry, compiler adapter, and catalog dependency. A
payload-sensitive wasm host can build one dialect with `--no-default-features`,
and verification compiles both single-dialect variants to make the composition
boundary mechanically enforceable.

**Backend inputs.** `solid_facts_backend::wire` owns the small interface that
orchestration receives from callers and Type Facts adapters: `SourceFile`,
`SourceChange`, and `TypeFactsProvider` (plus their grouped-demand and timing
value types). The crate root re-exports that interface for compatibility;
session lifecycle, caches, incremental rebuilding, and joining stay in the
orchestration implementation.

## Normalized package-contract seam

`solid-reactive-ir::contract_semantics` owns the rich package-behavior model.
It is a deep, wire-independent module: the public proposal contains exact
package, manifest, artifact-case, and export identities plus semantic values;
its private normalizer owns canonicalization, recursive traversal, guard
algebra, graph validation, contradictions, and semantic hashing. Compact wire
summary names, aliases, `closed` arrays, omission rules, and schema versions
must be expanded by `solid-facts-backend` before this seam. Analyzer consumers
must query `NormalizedContract` or the later accepted typestate, not decode
those wire conventions.

Knowledge is local to its immediate domain. `KnowledgeSet` distinguishes
unknown, partial positive, complete positive, and complete negative; recursive
tuple, object, array, choice, Promise, and AsyncIterable leaves carry their own
knowledge. Operations keep trigger, execution point, schedule, tracking,
ownership, and cardinality independent. Resources provide exact anchors for
owner, async, transition, cleanup, request/response, stream, and server-function
lifetimes. The normalizer refuses dangling identities, causal cycles,
overlapping or non-exhaustive closed guards, impossible capabilities, invalid
cardinality, and contradictory ownership instead of filling a missing premise
with a negative.

`ContractProposal::normalize` computes canonical semantic identity but does not
accept proposed closure. `AcceptedContract` deliberately has no public
constructor outside the Phase 11 proof authority.
`solid-facts-backend::contract_document_v2` owns the complete
temporary-schema decoder, summary expansion, local-closure interpretation,
resource limits, artifact-case identity derivation, and handoff into this
normalizer. `solid-facts-backend::contract_interface` now validates a
proof-issued receipt after exact artifact selection and is the only ordinary
analysis constructor of accepted typestate. `AcceptedContractIndex` retains
the exact importer/specifier binding, resolves exports through full runtime and
declaration identity, instantiates restricted guards from demand-shaped call
facts, and exposes only local normalized claim answers. Phase 14 removed the
legacy decoder and public contract types: ordinary native and WASM analysis now
accept only receipt-issued temporary-v2 inputs.

Phase 15 makes that boundary structurally bounded. Main documents, proof and
probe sidecars, workflow documents, probe transcripts, receipts, and accepted
catalogs all pass through one byte/depth/node/string-limited JSON decoder;
file-backed documents are size-checked before allocation. Artifact coordinates
and catalog/closure paths are canonical and traversal-free. Normalized causal
validation includes both explicit operation edges and operation triggers, while
resource lifetime dependencies are checked as their own acyclic graph. These
checks remain in the backend/normalized deep modules, so analysis consumers do
not acquire wire-limit, path, or schema mechanics.

`solid-facts-backend::artifact_resolution` owns the Phase 7 selection seam.
Host, Type Facts, and standalone acquisition all produce one exact
`ResolvedImport`; authority falls through in that order only when the stronger
source is unattested. An invalid or ambiguous stronger answer refuses the
import. The record separates runtime and declaration roots, branch traces, and
per-export targets, and binds a normalized proposal only when package,
manifest, artifact, declarations, closure, transform, entrypoint, and trace
agree with exactly one case.

The same module owns canonical dependency closure identity. Typed,
length-delimited hashing covers package-relative runtime/declaration files,
manifests and resolution inputs, literal chunks, materialized generated output
and transform identity, accepted dependency-contract edges, and explicit
opaque hazards. A hazard weakens only its named exports and immediate claim
domains; it cannot erase known positives or open an unrelated recursive leaf.
Ordinary Type Facts and WASM-host attestation preserve included paths, symlink
spellings, extensions, and both owning/resolver package versions. Native
discovery reads `.solid-checker/accepted-contracts.json`; WASM transports the
same exact document text, receipt text, and host resolution through
`acceptedContracts`.

`solid-facts-backend::proposal_generation` owns the Phase 8 replacement
generator seam. Exact Phase 7 resolutions select and bind every analyzed
artifact case before construction. The module withdraws every locally complete
knowledge set: positive items remain partial, negative candidates become
unknown, and each withdrawn leaf is retained as a proof obligation. It derives
local unresolved edges, positive-operation candidates, and witness-only probe
candidates in separate stages. `inferred_contract_v2` and `contract_workflow`
then normalize, merge artifact cases, encode deterministic temporary-v2
proposal bytes, and emit a proof plan whose acceptance is always unaccepted.

`packages/cli/scripts/generate-package-contract-v2.mjs` is the matching Node
orchestration seam. It acquires exact package artifacts and finite condition
partitions, invokes Rust for inference and normalization, and manages files and
processes without reading or rewriting semantic summaries. External dependency
proposals are never fed back as trusted closure.

`solid-reactive-ir::contract_semantics` also owns version-1 semantic claim
identity. A claim ID hashes exact package identity, artifact-case selection and
closure identity, exact export identity, and one validated normalized claim
path. It excludes JSON position, summary spelling, formatting, evidence
layout, and unrelated claim values. Domain claims and positive operation
existence have distinct typed paths; nonexistent operations, resources, or
recursive leaves cannot receive an ID.

`solid-facts-backend::evidence_sidecars` owns the Phase 9 evidence seam. It
derives artifact and claim bindings from the normalized model, then emits
separate deterministic `solid-checker-proof-evidence` and
`solid-checker-runtime-probe-evidence` version-1 documents. The proof family
records fact transcript, proof input, coverage, and tool identity. The probe
family records recipe, runtime/environment/sandbox, outcome, coverage, and
tool identity. Both bind semantic/package/artifact/declaration/transform/
closure identity per claim. The temporary main contract binds their content
hashes; sidecars bind normalized contract identity, avoiding a circular file-
hash dependency while still rejecting stale or cross-package evidence.

Sidecar validation refuses missing or unreferenced documents, content-hash
mismatch, wrong kind/version, stale semantic/package identity, cross-artifact
material, noncanonical claim IDs, duplicate claims, and claims absent from the
proposal plan. Its ordinary result retains only ordered semantic claim IDs. Raw
proof and runtime transcripts are neither stored in `AcceptedContract` nor
exposed to analyzer consumers; the Phase 11 verifier consumes bounded proof
material while issuing a receipt, and ordinary analysis remains offline after
those inputs are deleted.

`solid-facts-backend::runtime_probes` owns the semantic probe seam. Node is
responsible for package acquisition, worker process launch, and raw runtime
interaction; Rust owns every judgement over those observations:
exact artifact-case/mode planning, bounded time and semantic queue drains,
fresh process/realm/module identities, isolated deterministic repeats, event
and lifecycle validation, transcript identity, and authority classification.

Probe recipes come only from a `PlannedProposal`. A possible-positive recipe
must name one exact planned possible operation and marker. A closure-
falsification recipe must name one exact planned closure domain. Completed
repeats emit call, render, flush, callback, cleanup, settlement, emission,
transition, request, response, and stream events with zero-based semantic
sequence numbers. Cleanup, repeated AsyncIterable, transition, request/
response, and root-lifetime scenarios impose their own ordered lifecycle
checks. Runtime observations are grouped into exact claim-local modes in the
probe evidence sidecar, so refusal in one mode does not erase a valid sibling.

Only a repeated positive marker produces a witness or closure contradiction.
Missing markers, missing repeats, timeouts, errors, worker refusals,
environment mismatch, isolation reuse, excess drain, invalid lifecycles, and
nondeterministic event streams remain errors or local refusals. The module has
no negative, minimum, maximum, exhaustive, closure-acceptance, receipt, or
`AcceptedContract` output. Phase 11 is the first consumer allowed to replay a
contradiction record while proving closure.

`solid-reactive-ir::contract_semantics::proof` is the Phase 11 acceptance
authority and the only code that constructs `AcceptedContract`. Its public
input is raw, bounded rule material; successful `ReplayedProof` fields and the
accepted typestate remain private. Each replay binds one semantic claim to the
open normalized digest and exact package/artifact/declaration/closure/export
scope, requires a complete enumerated-versus-classified census with no
unresolved premise, and enforces the fact authority assigned to that proof
family. Package acquisition, Type Facts, compiler execution facts, accepted
dependency contracts, and runtime probes cannot substitute for one another.

Acceptance receives closure subjects derived by
`solid-facts-backend::proof_checker` from the Phase 8 `ProposalPlan`, never a
generator-authored accepted list. It rejects stale, orphaned, duplicate, or
missing family replays, policy downgrade, operation existence disguised as
closure, and every Phase 10 contradiction for a selected claim. It then closes
only the named call, operation-production, resource, guard, or recursive value
knowledge leaf and reruns normalization. Unknown siblings stay unknown and
partial siblings stay partial.

The verifier derives receipt-version-1 semantic, artifact, closure, proof, and
closed-claim roots with typed length-delimited SHA-256 domains. Proof and
census input order is irrelevant; transcript bytes, family, authority, claim,
scope, and policy remain identity. `BundledEvidenceStore` is immutable and
rehashes compiled receipt bytes. `LocalEvidenceStore` adds atomic,
content-addressed, idempotent receipt writes. Analyzer exposure through
`load_accepted_contract` now requires those receipt bindings to replay exactly.
The proof root remains opaque authority because raw proof material is outside
the analysis hot path; semantic, artifact, closure, and closed-claim roots plus
verifier policy are checked before any normalized query is exposed.

## How the Solid 1.x dialect landed

The sibling-directory shape sketched here before the 1.x dialect existed is
now the shipped layout, and the plan's items resolved as follows:

1. **Compiler adapter** — `solid-v1-compiler` implements
   `CompilerFactsProvider` over the `solid-1x-compiler` fork's trace, kept at
   differential parity with the Babel compiler Solid 1.x ships, exactly as
   `solid-v2-compiler` does for 2.0.
2. **Rule catalog** — `solid-v1-rules` projects the same `Program` onto the
   1.x catalog. The backend's snapshot, diagnostics, and LSP paths needed no
   changes, as intended.
3. **Dialect selection** — both dialects register in `dialect::ALL`; the
   entry points auto-detect from the resolved `solid-js` version and accept
   an explicit `--dialect`/`"dialect"` override. Cache and session keys carry
   the dialect id.
4. **The IR's Solid 2.0 coupling** — resolved by the `solid-dialect`
   vocabulary crate (ADR 0006): the primitive names, callback positions, and
   boundary tags that used to be hardcoded 2.0 knowledge are now questions
   the engine asks of the dialect it was handed.
5. **Wire-format coupling** — still open. `CompilerOptions` in
   `solid_facts::compiler` remains shaped around JSX compiler options
   (`effect_wrapper`, `hydratable`, `static_marker`). It is part of the
   CLI/wasm request schema, so generalizing it (for example into per-dialect
   opaque options) is a protocol change to make deliberately.

## Conventions

- Infrastructure crates never depend on dialect crates, with one deliberate
  exception: `solid-facts-backend` (the composition root) and
  `solid-checker-wasm` (an entry point) wire in the current dialect.
- Dialect crates depend only on `solid-facts` and `solid-reactive-ir`.
- New version-specific compiler adapters, vocabulary, rule identities, and
  wording go under `dialects/`. An analyzer that requires private
  `solid-reactive-ir` facts may remain in that crate only when its version
  ownership is explicit in the module name and it is gated by the selected
  `Version`; the dialect catalog still owns the external finding.
- Dialect-neutral enablement lives in `RuleOptions`; the Solid 1.x
  compatibility shapes are nested under its `Solid1xRuleOptions` member so
  the shared pipeline does not learn version-specific rule names.
