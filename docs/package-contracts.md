# Package contracts

Package contracts certify reactive behavior that TypeScript declarations do
not express: callback timing and tracking, ownership, operation cardinality,
resources and lifetimes, structured reactive values, and exact artifact
selection. Ordinary analysis consumes only proof-issued normalized contracts.
It never executes package code, reads proof sidecars, contacts a registry, or
accepts a proposal merely because it is present on disk.

The current public migration format is temporary `schemaVersion: 2`. It is the
only contract format produced or consumed by this checkout. The eventual
stable public schema is a later atomic cut; this repository does not maintain a
schema-1 compatibility decoder or emit both meanings in parallel.

## Trust boundary

One accepted input consists of three independently checked values:

- the exact JSON text of a normalized temporary-v2 main document;
- the exact JSON text of a proof-issued receipt bound to those document bytes;
- a `ResolvedImport` acquired by the host for one import occurrence, including
  exact package name, version, integrity, runtime and declaration artifacts,
  export identities, resolution traces, and the dependency-closure digest.

The native CLI discovers project inputs through
`.solid-checker/accepted-contracts.json`. WASM hosts pass the same three values
through `acceptedContracts`. The analyzer validates the wire document and
receipt, normalizes once, checks the exact import/artifact binding, and gives
analysis consumers only a wire-independent semantic index. Package names,
filenames, aliases, omitted fields, schema versions, and receipt structure stop
at that boundary.

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
[`semantic-model.md`](package-contract-v2/semantic-model.md). The temporary wire
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
generator enumerates a finite export map; wildcard exports are refused until
the caller supplies finite entrypoints. `--conditions browser,development`
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

## Proof verification and receipts

Only the Rust proof checker may turn proposed closure into accepted closure:

```sh
solid-checker contract verify solid-reactivity.json \
  --plan solid-reactivity.json.proposal.json \
  --proof proof-transcript.json \
  --artifact-case artifact-case:…
```

Verification replays every proof family demanded by each claim, checks complete
censuses and probe contradictions, finalizes the selected artifact case, and
writes an accepted document plus a receipt. The receipt binds at least:

- exact main-document bytes and normalized semantic digest;
- package, artifact-case, runtime/declaration, and closure identities;
- proof policy, verifier identity, proof roots, and the exact closed-claim root.

Changing document bytes, normalized semantics, artifact identity, closure,
proof policy, or a closed claim invalidates replay. Canonical semantic digests
sort unordered sets/maps, normalize equivalent restricted guards and numeric
spellings, preserve recursive leaf boundaries and all four knowledge states,
and exclude evidence-sidecar ordering or presentation. Semantically equivalent
normalizations have one digest; sibling or artifact changes do not.

Register accepted output explicitly rather than copying it into a magic
directory:

```json
{
  "format": "solid-checker-accepted-contract-catalog",
  "catalogVersion": 1,
  "contracts": [
    {
      "document": "contracts/example.accepted.json",
      "receipt": "contracts/example.receipt.json",
      "import": { "specifier": "example-package", "…": "full ResolvedImport" }
    }
  ]
}
```

Paths are resolved relative to the catalog. Duplicate exact import bindings,
missing files, byte drift, invalid receipts, unresolved exports, ambiguous
artifact selection, or mismatched package identity fail closed.

## Bundled first-party contracts

Receipt-issued Solid 1.x and Solid 2 RC.3 contracts live in both
`pkg/contracts/bundled/` and
`rust/crates/solid-dialect/contracts/`. Each dialect has a `bundle-index.json`;
runtime locks and dialect manifests bind the exact published authority. Run
`make contracts` to regenerate both physical sets and `make
contract-conformance` to verify byte identity, receipt roots, closure census,
and registry pins.

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
