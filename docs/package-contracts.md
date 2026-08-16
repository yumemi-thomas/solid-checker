# Package contracts

Milestone 5 introduces `solid-reactivity.json`, a non-executable summary that
preserves exported reactive reads when a dependency's implementation source is
not part of the consuming TypeScript project.

## Workflow

The package-level generator is the default workflow for libraries with export
subpaths:

```sh
solid-checker contract generate --package-root package
```

It derives package identity from `package.json`, walks the complete `exports`
map, expands wildcard subpaths, analyzes every supported ESM implementation
target that is present in the checkout independently, follows local `export *`
barrels, checks published modules through TypeScript's resolver, and
conservatively merges conditional builds into v1 `entrypoints`. Callability,
value-space references, resolved-call validity, selected declaration ownership,
argument-to-parameter mapping, and alias identity come directly from TypeScript
compiler facts. Runtime callback timing is applied only after an exact valid
signature has been selected; same-named user methods do not inherit
standard-library behavior. Package code is never imported or executed. From
the package directory the default output is `solid-reactivity.json`.

When conditional builds invoke the same callback parameter with different
timing, every observed timing mode is preserved. Consumers analyze each mode;
the merge does not discard one branch or treat the modes as contradictory.
When a conditional build changes a complete export summary (including removing
reactive behavior on the server), schema-v1 adds `variants`: each variant names
the conditions and the complete summary proven for that target. A consumer without
an explicit runtime-condition selector fails closed with an uncertifiable
environment-dependent contract result; it never applies the merged union.

Bundled conformance probes each applicable claim in client, server,
development, and production condition modes, and callback probes perform both
the initial and a subsequent update. A claim that passes only in one mode is a
conformance failure: the result is a surfaced environment mismatch, not a
reason to omit that mode or silently weaken the contract. The probe runner
records successful modes and call counts as row evidence only for claims that
already exist; it never writes newly observed behavior into a contract.

Use `--entrypoint ./state` to generate only one subpath while investigating a
failure, or `--conditions browser,import` to resolve the export map for a
specific environment. With no condition selector, every materialized supported
ESM target is checked. This permits generation from a source checkout whose
export map also advertises build outputs that have not been produced yet; a
package with no materialized runtime target still fails. Compatible facts are
unioned; callability is retained only when it is valid in every selected
runtime target, and genuinely incompatible return or async summaries stop
generation.

An application developer can generate a project-owned contract without
modifying `node_modules`:

```sh
solid-checker contract generate \
  --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
```

Generated TypeScript projects live in the OS temporary directory and are
removed after each entrypoint; the analyzer retains the package root separately
for export filtering and module resolution. Package code is analyzed statically
and is not executed. CJS-only entrypoints currently fail as unsupported instead
of receiving an inferred empty summary.

`contract generate` also writes a sibling `<contract>.review.md` checklist. It
calls out runtime entrypoints with no generated summary, function exports with
no callback execution row, inherited claim rows, and entrypoints whose
conditional environment selection needs review. The checklist is intentionally
separate from `solid-reactivity.json`: generation never promotes inferred
claims or auto-resolves the items it lists. Stdout remains one line and names
both output paths.

The lower-level single-project workflow remains useful for packages without a
`package.json#exports` map:

Analyze the package and emit its solved exported function summaries:

```sh
solid-checker --project package/tsconfig.json \
  --emit-contract package/solid-reactivity.json \
  --package-name reactive-package \
  --package-version 1.0.0 \
  --declaration-artifact package/index.d.ts \
  --implementation-artifact package/index.js
```

Load the contract while analyzing a consumer:

```sh
solid-checker --project app/tsconfig.json \
  --contract package/solid-reactivity.json \
  --format json \
  --certify
```

`--contract` is repeatable. Contracts published as
`node_modules/<package>/solid-reactivity.json` are discovered automatically,
including contracts for package subpaths. Schema v1 records an `entrypoints`
map keyed exactly like `package.json#exports` (`"."`, `"./state"`,
`"./server-functions"`, and so on). Explicit contracts override discovered and
bundled contracts. The loader binds an import to its exact entrypoint and
export through Type Facts; it does not fall back to the root entrypoint.

The on-disk format stores each distinct effect summary once. Entrypoints group
export names by summary identifier, and an identical subpath surface can use
`sameAs`:

```json
{
  "schemaVersion": 1,
  "package": { "name": "reactive-package", "version": "1.0.0" },
  "compilerFactsProtocol": 1,
  "summaries": {
    "function": { "kind": "function" },
    "function-1": {
      "kind": "function",
      "callbacks": [{ "parameter": 0, "execution": "tracked" }]
    }
  },
  "entrypoints": {
    ".": {
      "exports": {
        "function": ["createRoot"],
        "function-1": ["createMemo"]
      }
    },
    "./client": { "sameAs": "." }
  },
  "evidence": { "kind": "inferred", "generator": "solid-checker" }
}
```

This normalization is only a wire-format concern. The loader expands it to the
full entrypoint/export model before resolution and analysis, so summaries remain
as expressive as before without repeating `{ "kind": "function" }` hundreds of
times. Summary identifiers are document-local and have no semantic meaning.

Every contract requires a package version and is accepted only when that exact
version is installed. A bundled beta-31 contract therefore cannot silently
certify beta 30, a later beta, or Solid 1.x.

Application developers can also maintain a contract when a package does not
publish one. Put it at:

```text
.solid-checker/contracts/<package>/solid-reactivity.json
```

Scoped names retain their directory structure, for example
`.solid-checker/contracts/@scope/package/solid-reactivity.json`. Project-owned
contracts are discovered automatically and override contracts from
`node_modules`; an explicit `--contract` still has the highest precedence.
The same `--emit-contract` workflow can generate this file when the package
source and a TypeScript project for it are available, or it can be authored
against the contract schema and checked with `--validate-contract`.

Before checking, inspect imported Solid-dependent packages and their contract
coverage:

```sh
solid-checker --project app/tsconfig.json --check-contracts
```

The command reports bundled, published, local, explicit, unverified, and
missing contracts. It exits with status 1 when a package whose manifest
depends on or peers with Solid has no certifiable contract.

Normal analysis performs the same completeness check. A missing contract emits
the uncertifiable `SC9005 package-contract-missing` finding at the package
import, changes the snapshot status to `uncertifiable`, and causes `--certify`
to exit with status 1. This behavior is shared by one-shot and retained-daemon
checks. Use `--check-contracts` when only the focused coverage report is needed.

Validate contracts and their artifacts without opening a TypeScript project:

```sh
solid-checker --validate-contract package/solid-reactivity.json
```

## Trust boundary

The schema is [solid-reactivity.schema.json](../schema/solid-reactivity.schema.json).
The loader fails closed on:

- unsupported schema or compiler-facts protocol versions;
- unknown JSON fields or malformed summaries;
- missing or unused summary identifiers, duplicate exports, and entrypoint
  alias cycles;
- unsupported effect or evidence kinds;
- imports of entrypoints or exports missing from an otherwise valid contract;
- unsafe artifact paths; and
- declaration or implementation hashes that do not match the files beside the
  contract.

Artifact hashes use `sha256:<lowercase hex>`. The artifact flags hash exact file
bytes and require each file to be inside the emitted contract's directory.
Artifacts remain optional because they are not always available at emission
time, but they are verified whenever present. The contract itself is SHA-256
hashed when loaded, and that identity is included in the certification package
summary.

The bundled Solid 2 artifacts also ship
`pkg/contracts/bundled/runtime-lock.json`. It records the resolved version and
npm integrity for every dependency and peer edge in the audited
`solid-js`/`@solidjs/web` runtime closure. Conformance checks the installed
manifests, resolved versions, and integrities against that lock; the
`^2.0.0-rc.0` declaration for `@solidjs/signals` therefore cannot drift without
failing the gate.

Changes under `rust/crates/solid-reactive-ir/` run the bounded package-contract
torture corpus in `.github/workflows/contract-corpus.yml`. The corpus covers
runtime-mutated namespaces, conditional semantic branches, getter-backed
exports, deep re-export barrels, and declaration/runtime disagreement. Its
checked-in expected outputs are reviewed like snapshots: an unexplained drift
fails the engine-change gate, and the runner never updates those pins.

Evidence is enforced, not decorative. Contracts emitted by this CLI use
`inferred`; consumers report them as `unverified` and cannot certify through
them. Certification accepts `verified`, `reviewed`, `trusted`, and `attested`
contracts. Legacy `generated` remains parseable but is also unverified.

Schema-v1 contracts may also put `evidence` on an export summary, reactive-read
row, callback row, or recursive return row. Row evidence is one of `inferred`,
`probed`, `reviewed`, or `inherited-from`; probed rows record `modes` and a
positive `calls` count, while inherited rows record the exact `package` and
`version`. Contracts without row evidence retain the contract-level behavior.
When row evidence is present, certification additionally rejects any inferred
row so a promoted contract cannot hide an uncertified claim inside a verified
document.

Promote an inferred contract only after checking it against the exact package
artifacts and reviewing every unresolved behavior. `verified` means mechanical
artifact/surface/behavior checks passed; `reviewed` records an explicit human
review; `attested` is reserved for a verifier-produced release identity.

## Effect summaries

The schema records:

- direct reactive accessor and store-path reads;
- accessor and store returns, including factory-to-factory propagation;
- tuple slots and named object properties containing reactive returns;
- inline, tracked, and deferred callback parameters;
- Promise and async-iterable behavior;
- inert exported values; and
- inferred, verified, reviewed, trusted, or attested evidence.

Generation covers function declarations, exported arrows, overloads, nested
generics, async functions, multiple const declarations, classes, re-exports,
aliases, and subpath imports. Consumers support named imports and local aliases.
Calls in compiler-tracked JSX retain their tracked status; calls in ordinary
function bodies produce `strict-read-untracked` findings.

Structured returns are an additive part of `schemaVersion: 1`. A tuple uses an
`elements` array (with `null` for an uncontracted slot), while an object uses a
`properties` map. Leaves retain the existing `accessor` or `store-path` shape.
Consumers recognize those leaves through array/object destructuring and direct
object member access. An `argument` return identifies a parameter whose actual
value is returned unchanged; consumers instantiate it at each call, so generic
identity and invariant wrappers preserve nested tuple/object reactivity without
inventing a new schema version.

A shorthand property (`{ pathname }`) writes one identifier where a key and a
value both stand, and TypeScript answers a symbol query at that span with the
*property's* symbol, never the value binding's. The value is identified instead
by the binder's resolution of that exact reference, carried on the object
property fact as `shorthandBinding`. That is scope-exact: a same-spelled
binding in a sibling block declares a different symbol at a different span, so
it can neither be chosen nor make the visible declaration ambiguous. A
shorthand the binder resolves to no declaration in this file -- a global, or an
import specifier, including one for an accessor declared in another file --
carries no fact and yields no structured property.

Generation fails closed when an exported parameter escapes through an
uncontracted external call whose execution semantics are unknown. This includes
forwarding a callback to `queueMicrotask`, `Reflect.apply`, or an uncontracted
scheduler: emission stops instead of producing an empty, falsely inert summary.
Local calls are summarized transitively, and forwarding into known Solid
callback slots records the corresponding tracked, deferred, or inline
execution.

Local deferred-flow proofs are structural rather than name-based. A function
installed on an object is considered deferred only when that object is
caller-owned or returned. A callable constructor parameter is considered
retained only when it is a TypeScript parameter property on an object passed to
an exact compiler-resolved retaining runtime position, such as a Proxy handler.

## Bundled and ecosystem contracts

Verified contracts for `solid-js` and `@solidjs/web` are embedded in the
checker and selected automatically from project imports. They pin Solid
`2.0.0-rc.0` and its npm integrity. The core contract covers the root and
refresh entrypoints; the web contract covers all 13 runtime entrypoints,
including server-functions, frames, serialization, storage, and the JSX
runtimes.

Run the exact-release conformance suite with:

```sh
make contract-conformance
```

It enumerates every non-pattern runtime entrypoint and conditional ESM build,
checks missing/stale exports and function/value kinds, verifies npm version and
integrity, and requires a passing behavioral probe for every callback and
reactive-return claim. Behavioral probes run the applicable client, server,
development, and production conditions independently, exercise both the first
and subsequent callback calls, and report their mode and call count. A claim
must pass in every mode selected by its entrypoint conditions. `--write` may
record those passing modes as `probed` row evidence; it never discovers or
adds an uncontracted behavior. The normal `scripts/verify.sh` workflow runs
its conformance half (`scripts/check-bundled-contracts.mjs`); CI's contracts
job runs the full suite on every push and pull request.

A bundled contract for Solid 1.x is embedded alongside them:
`pkg/contracts/bundled/solid-v1/solid-js.json` pins `solid-js` 1.9.14 and covers the
`.`, `./store`, and `./web` entrypoints. It is generated by
`scripts/generate-bundled-solid1-contract.mjs` from two checked-in inputs: the
per-subpath export census `pkg/contracts/bundled/solid-v1/solid-js-census.json`,
which decides which exports exist under which entrypoint, and the reviewed
semantics map `rust/crates/solid-dialect/contracts/solid-v1/solid-js.json`, which
supplies the callback and return summaries. The same
`make contract-conformance` target runs the generator with `--check`, which
fails when the checked-in artifact is stale relative to either input instead
of shipping a drifted contract.

Solid 1.x also embeds a narrow reviewed contract for
`@solid-primitives/scheduled@1.5.3`. It distinguishes deferred `debounce`,
`throttle`, and `scheduleIdle` callbacks from the inline scheduler factory
arguments used by `leading`, `leadingAndTrailing`, and `createScheduled`. The
contract is exact-version matched; other releases must ship or generate their
own contract rather than inheriting guessed timing.

The pinned Solid Primitives `next` corpus contains 98 packages. Its contracts
are generated from complete package export maps, including subpaths and
materialized conditional targets. Regenerate and validate the complete corpus
with:

```sh
make corpus
```

Set `SOLID_PRIMITIVES_CORPUS=/path/to/clean/checkout` to reuse a local clone.

Generation automatically discovers contracts from declared dependencies and
repeats to a fixed point so their transitive summaries are retained. Validation
checks every normalized contract and confirms each package manifest publishes
`solid-reactivity.json`; artifact hashes are also checked whenever a contract
contains them.
