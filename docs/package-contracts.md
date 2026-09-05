# Package contracts

Package contracts certify reactive behavior that TypeScript declarations do
not express: callback timing and tracking, ownership, operation cardinality,
resources and lifetimes, structured reactive values, and exact artifact
selection. For external packages, ordinary analysis consumes only proof-issued normalized contracts.
It never executes package code, reads proof sidecars, contacts a registry, or
accepts a proposal merely because it is present on disk.

Solid core is the built-in runtime foundation: `solid-js`, `@solidjs/signals`,
and `@solidjs/web` obtain their modeled behavior from the selected Solid
dialect. Their contract entries do not supplement or override ordinary
analysis, including entries imported through an alias. These are reviewed
runtime premises, not an independent certification of Solid's implementation.
Missing native behavior remains unknown. The historical core bundle indexes
are empty; retained main documents are proposals and conformance material.
ADR 0027 records the migration and its applicability boundary.

The public format is stable `schemaVersion: 1` with the required
`format: "solid-reactivity-contract"` discriminator. It is the only contract
format produced or consumed by this checkout. The repository maintains neither
a temporary-v2 decoder nor compatibility with the retired legacy-v1 shape.

## Trust boundary

One accepted policy-2 input consists of four independently checked values:

- the exact JSON text of a normalized stable-v1 main document;
- the exact canonical JSON text of a receipt-version-2 payload bound to those
  document bytes and every certification root;
- explicit immutable built-in provenance or a configured persistent-local or
  portable Ed25519 trust-store entry, including policy/build constraints and
  revocation epoch;
- a `ResolvedImport` acquired by the host for one import occurrence, including
  exact package name, version, integrity, runtime and declaration artifacts,
  export identities, resolution traces, and the dependency-closure digest.

The native CLI discovers project state through
`.solid-checker/accepted-contracts.json`. During the Phase 19 atomic cut,
catalog version 2 records retired policy-1 entries only as
`status: "obsolete-policy1"`; those entries produce an explicit uncertifiable
result and carry no receipt bytes. WASM `acceptedContracts` similarly refuses
policy 1 and raw policy-2 bytes without issuer provenance. The authenticated
loader normalizes once, checks exact import/artifact binding, and gives analysis
consumers only a wire-independent semantic index.

An installed `solid-reactivity.json` without a matching receipt and exact
catalog entry is a proposal, not evidence. A same-named package, declaration
stub, local link, alternate conditional export, or byte-different artifact
cannot borrow another artifact's accepted semantics.

## Semantic model

Knowledge is local to the smallest claim domain or recursive value-shape leaf:

- `Unknown` means no usable premise exists.
- `Partial` carries known members without claiming an exhaustive census.
- `CompletePositive` proves the complete set and that it is non-empty.
- `CompleteNegative` proves the complete set is empty.

Missing evidence therefore never becomes negative proof. An open callback
leaf does not erase a known return shape; an unknown tuple member does not
contaminate its siblings; and a complete-negative cleanup claim says more than
an absent cleanup claim.

A present but unresolved runtime export-kind answer produces `shape: "unknown"`
with all behavioral domains open. It proves neither callability nor
non-callability and contributes nothing to `exportsProven`. Missing kind facts
still refuse. Exact export binding and executable dependency closure remain
required, while independent exports can establish their own claims (ADR 0011).
Accepted-contract projection preserves this unknown shape through re-exports.

Parameter-member reads also retain their execution context during generation.
A read found inside a nested callable does not establish a same-stack read by
the exported function. Until its execution can be represented, the compact
inference model leaves that export's reads unknown, including mixed direct and
captured reads. Direct-body read proposals still require native evidence, and
reintroducing an unsupported captured read still refuses (ADR 0013).

Local-helper callback forwarding likewise requires an established enclosing
execution chain reaching the parameter's declaring function. An opaque wrapper
or a stored arrow cannot inherit the helper's same-stack timing. Those callback
domains remain unknown; direct calls retain their proposals and native proof
demands (ADR 0014).

Returning a callable also does not prove that its lexical descendants execute.
Deferred callback proposals require the exact containing callable to escape
through the return value; unproven descendant chains stay unknown (ADR 0015).
Inside primitive-defining packages, callback proposals are withheld wholesale
because compact summaries lose bootstrap provenance. This includes otherwise
independent direct callbacks; it never asserts callback absence (ADR 0017).

A generic return can still prove an exact input/output identity. Protocol 16's
authenticated return-site parameter fact requires an unchanged whole binding;
the verifier requires complete control flow and agreement of all possible
returns. It does not infer a concrete shape or callability from that identity
(ADR 0016).

Dependency-graph certification acquires static runtime imports as well as
re-exports through exact archive and lock identities. Declaration-only edges
retain their compiler-source role; dynamic and unresolved edges remain open.
Both graph request forms forward the requested pinned probe configuration, so
an addressed closure must pass its census and veto before receipt issuance
(ADRs 0018–0019).

Exports contain exact operation nodes and causal edges. Trigger, schedule,
tracking, ownership, and possible-versus-guaranteed cardinality are independent
axes. Owner requirements, owner production, and owner kind are independent too.
Operations may create, use, or dispose resources whose lifetimes are explicit.
Restricted guards form finite, validated partitions over exact artifact or call
facts. When guard selection is unresolved, the consumer performs a monotone
join: possible behavior can grow, but guaranteed behavior and negative proof
cannot be invented.

Every artifact case binds package identity, entrypoint, runtime and declaration
files, closure identity, and exact export identity. Experimental status is
local to its case or export. Validation rejects contradictions, dangling graph
references, cycles where forbidden, invalid resource states, overlapping guard
partitions, noncanonical claim paths, and false closure.

The authoritative model is documented in
[`semantic-model.md`](package-contract-v2/semantic-model.md). The stable wire
format is an encoding of that model, not the interface analysis code uses.

## Generate and review a proposal

Generation is output-neutral and does not execute the package:

```sh
solid-checker contract generate \
  --package-root node_modules/example-package \
  --integrity 'sha512-…' \
  --output .solid-checker/contracts/example-package/solid-reactivity.json
```

`--integrity` is required because exact registry identity must not be inferred
from a package manifest. `--entrypoint ./subpath` is repeatable. Without it the
generator enumerates a finite export map. A wildcard whose export key and
target each have one statically enumerable `*` is expanded from the exact
package file census; ambiguous, empty, symlinked, or oversized wildcard
surfaces remain local refusals. `--conditions browser,development`
selects one exact runtime environment. When no condition list is supplied, the
generator enumerates a bounded finite partition and refuses an unbounded one.

Rust owns inference, normalization, proposal-plan construction, and multi-case
merging. JavaScript owns package resolution and process/file lifecycle only.
Generation records independently known semantics and leaves every unresolved
recursive leaf open. External export-all boundaries without independently
accepted semantics are refused; a newly generated dependency proposal cannot
be used to close the parent proposal.

Inspect recursively open claims without changing the proposal:

```sh
solid-checker contract review solid-reactivity.json
```

The deterministic review document lists exact artifact cases, exports, claim
paths, and local experimental state. Review never closes a claim or issues a
receipt.

Use `solid-checker contract check --project tsconfig.json` to report bundled,
accepted, unverified, stale, unbound, and missing package state. A missing sweep
can generate proposals for registry-installed packages that carry exact
integrity:

```sh
solid-checker contract generate --missing --project tsconfig.json
```

Linked/local packages without registry integrity remain explicitly
uncertifiable; the sweep does not invent an identity or overwrite an existing
proposal under review.

## Runtime probes

Runtime probes are opt-in falsifiers. They execute exact package code in fresh
worker processes only through `solid-checker contract probe`; ordinary analysis
and generation never do so.

```sh
solid-checker contract probe solid-reactivity.json \
  --request probe-request.json
```

Rust authorizes a claim-addressed plan and classifies raw events. The Node
driver owns fresh-process execution. Exact environment, artifact mode, recipe,
plan, and producer identities are bound into the result. Timeout, error,
environment mismatch, inconsistent repetitions, or finite non-observation stay
local refusals. A contradiction can block one proposed closed claim; a passing
probe can never establish closure.

## Policy-2 certification and receipts

Caller-authored proof transcripts no longer issue receipts. The former
`contract verify --proof` route deterministically refuses, and receipt version
1 is obsolete. Rust derives a policy-2 demand graph, verifies each applicable
witness through its owning producer, treats mandatory probes only as a veto,
and invokes a configured issuer only after the complete graph closes. A
receipt-version-2 payload binds exact main/semantic identity; every artifact,
producer, dependency, probe, witness, and closed-claim root; verifier
source/build; and issuer kind, scope, key, algorithm, and signature.

The automatic `solid-checker contract certify --integrity 'sha512-…'`
orchestration reacquires exact registry metadata and archive bytes, generates
the open proposal, and asks Rust to replay the snapshot and derive demand IDs.
It accepts no proof or receipt input. Missing live Type Facts, compiler,
dependency, probe, or issuer authority becomes a deterministic demand-local
refusal; an optional audit transcript is marked non-authoritative and
non-replayable. The transaction cannot publish a catalog pointer until every
witness, certification result, and receipt succeeds. The current policy-1
migration catalog shape is deliberately refusal-only:

```json
{
  "format": "solid-checker-accepted-contract-catalog",
  "catalogVersion": 2,
  "contracts": [
    {
      "document": "contracts/example/solid-reactivity.json",
      "status": "obsolete-policy1",
      "import": { "specifier": "example-package", "…": "full ResolvedImport" }
    }
  ]
}
```

Paths are resolved relative to the catalog. An obsolete-policy entry supplies
no semantics; it preserves the exact import demand and reports why the prior
claim cannot be used.

## Bundled first-party contracts

The checked Solid 1.x and Solid 2 RC.3 main documents remain proposal/audit
oracles in both `pkg/contracts/bundled/` and
`rust/crates/solid-dialect/contracts/`. Their active bundle indexes are empty:
all 24 former first-party artifact cases lost policy-1 authority at the atomic
cut. The mandatory probe harness is now bound
(`docs/adr/0006-probe-harness-binding.md`), so a reissue is no longer blocked
on that. Two things still block it: a hand-authored recipe for every closed
claim domain each case proposes, because a scheduled veto with no recipe
refuses; and, for the behavioral call domains these cases actually propose, the
implementation-census premise the Type Facts closure witness does not yet have.

The bundle is selected by exact installed artifact identity and environment,
never by package name. Reduced fixture stubs and byte-different local copies are
expected refusals even when their exports have familiar names.

## Failure policy

The checker reports a violation only from proved semantic facts. An unresolved
claim, artifact, export, guard, dependency boundary, or receipt is an
uncertifiable result scoped to that exact demand. Unrelated closed facts remain
usable. TypeScript-owned errors are never duplicated, and probes or generator
coverage never weaken the proof requirements needed for automatic package
verification.

## Published JavaScript graph probes (2026-09-04)

ADRs 0020–0021 allow a live-verified independent creates census to compose with
authenticated dependency receipts while retaining the parent's mandatory veto.
The graph passes exact dependency snapshots into the private workspace; sandbox
scheme 7 binds their materialization manifest and refuses any dependency whose
actual Node conditions select different bytes. This does not substitute JS for
a TS-source claim. Five Kobalte 0.9.2 creates closures are measured in the
explicit browser root case; see docs/2026-09-04-published-js-probe-unlock.md.

ADR 0022 makes declaration target selection continue to a later active types
branch only after an earlier target has no declaration candidate. Runtime
selection is unchanged. ADR 0023 distinguishes collection storage from deferred
callback invocation; storage leaves callbacks unknown.

## Controlled TypeScript erasure execution (2026-09-05)

ADR 0026 adds `node-strip-inert-esm-v1`; ADR 0028 adds
`node-strip-import-free-esm-v1` for a directly exported callable whose module
has no runtime imports and whose TypeScript syntax can be removed without
changing any runtime token. ADR 0030 adds
`node-strip-relative-ts-graph-esm-v1` for a finite graph of package-local `.ts`
modules whose exact relative edges are replayed by the authenticated native
resolver. Native census and a mandatory derived-byte veto
must both succeed. A fresh native-owned launch consumes the scoped receipt by
importing the verified derived ESM at the exact source URL. The inert profile
calls its export with no arguments; the import-free profile replays the exact
recipe that selected the closure. Every repeat uses a fresh worker. No function
or acceptance token escapes into a caller's runtime.

The Rust API is `CertificationPlan::certify_and_execute`. The native
`--execute-contract-certification <request.json>` boundary supports inert
execution request version **6**, import-free request version **7**, and
relative-graph request version **8**, each
with one `planning` and the corresponding `executionProfile`. The planning, archive,
Type Facts, issuer and probe-corpus inputs are the same authenticated inputs
as a version-1 single-case certification request. The profile is a request;
the native parser, live producer, compiled pins, actual transformer output
and launcher must establish every premise themselves.

Versions 6 through 8 write `<catalogRoot>/controlled-execution.json`, a separate
`solid-checker-controlled-type-erasure-execution` result, and publish no accepted
catalog or ordinary trust configuration. Its receipt is version **5**, with
signature domain `solid-checker:controlled-execution-receipt:v5`, explicit
profile/proof identity and all native proof and execution bindings. It contains
no extractable signed version-2 receipt. The existing analyzer refuses it.
Receipt replay is unsupported: each invocation requires a fresh native proof
and authenticated execution transaction. This first consumer is intended to
make execution applicability concrete, not to assert compatibility with an
application's TypeScript compiler, bundler, browser or Node installation.

Sandbox scheme **10** and worker protocol **v5** bind the narrowly scoped loader
override, parser preservation relation, recipe replay, every source/output pair,
the exact relative edge map, and the consumer report. Ordinary published-JS
behavior and ordinary TS-source refusal are unchanged. Browser-dependent
functions require a separately reviewed profile; no fake globals are installed.
