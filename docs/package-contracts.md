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

Evidence is enforced, not decorative. Contracts emitted by this CLI use
`inferred`; consumers report them as `unverified` and cannot certify through
them. Certification accepts `verified`, `reviewed`, `trusted`, and `attested`
contracts. Legacy `generated` remains parseable but is also unverified.

Promote an inferred contract only after checking it against the exact package
artifacts and reviewing every unresolved behavior. `verified` means mechanical
artifact/surface/behavior checks passed; `reviewed` records an explicit human
review; `attested` is reserved for a verifier-produced release identity.

## Effect summaries

The schema records:

- direct reactive accessor and store-path reads;
- accessor and store returns, including factory-to-factory propagation;
- inline, tracked, and deferred callback parameters;
- Promise and async-iterable behavior;
- inert exported values; and
- inferred, verified, reviewed, trusted, or attested evidence.

Generation covers function declarations, exported arrows, overloads, nested
generics, async functions, multiple const declarations, classes, re-exports,
aliases, and subpath imports. Consumers support named imports and local aliases.
Calls in compiler-tracked JSX retain their tracked status; calls in ordinary
function bodies produce `strict-read-untracked` findings.

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
`2.0.0-beta.31` and its npm integrity. The core contract covers the root and
refresh entrypoints; the web contract covers all 11 runtime entrypoints,
including server-functions, frames, serialization, storage, and the JSX
runtimes.

Run the exact-release conformance suite with:

```sh
make contract-conformance
```

It enumerates every non-pattern runtime entrypoint and conditional ESM build,
checks missing/stale exports and function/value kinds, verifies npm version and
integrity, and requires a passing behavioral probe for every callback and
reactive-return claim. The normal `scripts/verify.sh` workflow runs its
conformance half (`scripts/check-bundled-contracts.mjs`); the generated-file
drift half (`generate-bundled-solid1-contract.mjs --check`) runs in CI's
contracts job.

A bundled contract for Solid 1.x is embedded alongside them:
`pkg/contracts/bundled/solid-js-v1.json` pins `solid-js` 1.9.14 and covers the
`.`, `./store`, and `./web` entrypoints. It is generated by
`scripts/generate-bundled-solid1-contract.mjs` from two checked-in inputs: the
per-subpath export census `pkg/contracts/bundled/solid-js-v1-census.json`,
which decides which exports exist under which entrypoint, and the reviewed
semantics map `rust/crates/solid-dialect/contracts/solid-js-1x.json`, which
supplies the callback and return summaries. The same
`make contract-conformance` target runs the generator with `--check`, which
fails when the checked-in artifact is stale relative to either input instead
of shipping a drifted contract.

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
