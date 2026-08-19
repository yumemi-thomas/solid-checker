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

## Checking contract coverage and freshness

Inspect imported Solid-dependent packages and their contract coverage:

```sh
solid-checker contract check
```

`solid-checker --project app/tsconfig.json --check-contracts` is the same
report; `contract check` accepts the same `--project`, `--format`, and
`--contract` options and defaults to `tsconfig.json` and text output. Each
package is reported as exactly one status:

| Status | Meaning | Certifies |
| --- | --- | --- |
| `bundled` | This checker's own audited contract matches the installed version. | yes |
| `published` | The package ships a contract for its installed version. | yes |
| `local` | A project-owned contract under `.solid-checker/contracts/`. | yes |
| `explicit` | A contract passed with `--contract`. | yes |
| `unverified` | A contract whose evidence is `inferred`; its claims were never reviewed. | no |
| `stale` | A contract that describes a **different version** than the one installed. | no |
| `missing` | No contract for a package whose manifest depends on or peers with Solid. | no |

Every non-certifying status prints the action that resolves it, and the command
exits with status 1 when any package needs action, so it works as a CI gate.
The JSON format reports, per package, a `remedy` field carrying the same action
and a `detail` field naming the reason when the status alone does not say it
(the two disagreeing versions behind `stale`, the evidence kind behind
`unverified`). Both are omitted for a status that certifies. The report also
carries `missing` (the count of packages needing action) and `stale` (the drift
subset of that count).

### Stale contracts

A contract names the exact package version it was generated and reviewed
against. When the installed version moves — an upgrade, a lockfile refresh, a
different resolution — the contract stops being evidence about the package the
project actually has, and the checker refuses to apply it.

For a project-owned or published contract, the remedy is to regenerate and
re-review it:

```sh
solid-checker contract generate --package-root node_modules/reactive-package \
  --output .solid-checker/contracts/reactive-package/solid-reactivity.json
```

Regeneration rewrites the contract and its review checklist; the checklist
still has to be reviewed, because generation never promotes inferred claims to
reviewed ones. For a *bundled* contract the remedy is different and the report
says so: the consumer does not own that artifact, so the options are to install
the version this checker audited or to upgrade `solid-checker` to a release
that audits the installed one.

Analysis fails closed on the contract without failing the run. The stale
contract is refused — a contract for another version is not weaker evidence, it
is evidence about a different artifact — and the package is reported exactly as
an uncontracted one: an uncertifiable `SC9005 package-contract-missing` finding
at the package import, snapshot status `uncertifiable`, and `--certify` exiting
1. The message states which case applies, naming both versions rather than
claiming no contract exists, and the hint carries the same remedy the report
prints.

Refusing the contract without stopping the run is what keeps one upgraded
dependency from blanking out every other finding in the project, which matters
most in an editor. It does not weaken enforcement: the project cannot certify
until the contract is regenerated and reviewed.

A *malformed* contract — unparseable, wrong schema version, wrong package name,
mismatched artifact hashes — still fails the analysis outright. That is a broken
file rather than drift, and no finding can describe a document the loader could
not read.

Missing and unverified contracts take the same path and have always done so.
This behavior is shared by one-shot and retained-daemon checks. Use
`contract check` when only the focused coverage report is needed.

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
shorthand the binder resolves to a relative named/default import is followed
through exact project-local ESM exports (including re-export chains) before a
reactive leaf is claimed. Ambiguous relative targets, bare/path-mapped imports,
namespace imports, globals, and unresolved cycles yield no structured property;
the generator never chooses a same-spelled declaration or filesystem candidate
as a substitute for exact resolution.

Generation fails closed when an exported parameter escapes through an
uncontracted external call whose execution semantics are unknown. This includes
forwarding a callback to `queueMicrotask`, `Reflect.apply`, or an uncontracted
scheduler: emission stops instead of producing an empty, falsely inert summary.
Local calls are summarized transitively, and forwarding into known Solid
callback slots records the corresponding tracked, deferred, or inline
execution.

Schema-v1 callback entries describe execution timing, not the callback's owner
capability. When an exported function invokes a caller-supplied callback from a
Solid leaf owner such as `onSettled`, recording only `"execution": "deferred"`
would lose the fact that cleanup, flush, and nested primitive creation are
forbidden there. Generation therefore emits SC9012 and refuses to certify that
surface until an exact in-project callback discharges it or a future
backward-compatible contract field can represent the leaf-owner constraint.

Local deferred-flow proofs are structural rather than name-based. A function
installed on an object is considered deferred only when that object is
caller-owned or returned. A callable constructor parameter is considered
retained only when it is a TypeScript parameter property on an object passed to
an exact compiler-resolved retaining runtime position, such as a Proxy handler.

## Adding a package to a dialect

This section is for maintainers of this repository, adding a package that a
dialect models directly. Application developers generating a contract for a
dependency want [Checking contract coverage and
freshness](#checking-contract-coverage-and-freshness) instead.

### The generate/check model

Dialect contract artifacts are **derived from a declaration plus an installed
package**, and every one of them is checked by regenerating it and comparing:

```sh
make contracts        # write the artifacts
make contracts-check  # regenerate into memory and fail on any difference
```

`make contracts-check` runs in CI's `rust-engine` job on every push and pull
request, after installing the exact pinned releases. A checked-in artifact that
no longer matches what the generator produces from the pinned package is a
failure, not something the next run quietly fixes. Adding a package therefore
means adding its declaration; the artifacts follow from it, and the gate keeps
them honest.

**Only half of a contract is derived, and the halves are checked differently.**
The *export set* is a syntactic fact read from the package's declarations with
the same parser the checker runs on user code, following `export *` and
`export { x } from` chains — so drift in it is caught mechanically. The
*reactive semantics* — whether a function opens a root, establishes an owner,
returns a live store or a snapshot — cannot be derived from a signature and are
hand-authored tables inside `solid-contract-gen`, each carrying its evidence.
`--check` proves the artifact matches the tables; it cannot prove the tables
match the runtime. That is what the runtime probes below are for, and why a
version bump is a re-audit rather than a regeneration.

### Declaring the package

Add one entry to the `contracts` array of `rust/dialects/<id>/dialect.json`:

```json
{
  "package": "@solidjs/web",
  "packagePathEnv": "SOLID_V2_SOLIDJS_WEB_PACKAGE",
  "defaultPackagePath": "node_modules/@solidjs/web",
  "generatorTarget": "solid-v2/solidjs-web",
  "reviewContract": "rust/crates/solid-dialect/contracts/solid-v2/solidjs-web.json",
  "exportsIndex": "rust/crates/solid-dialect/src/exports/solid_v2_solidjs_web.rs",
  "bundledContract": "pkg/contracts/bundled/solid-v2/solidjs-web.json",
  "probeRuntime": true
}
```

Every field except `probeRuntime`, `composeScript`, and `composeInputs` is
required, `generatorTarget` must start with `<id>/`, and no two entries may
share a `generatorTarget` or declare the same package twice.

A contract that is **reviewed against a package rather than derived from it**
declares `"generated": false` and carries only `package` and `bundledContract`:

```json
{
  "package": "@solid-primitives/scheduled",
  "bundledContract": "pkg/contracts/bundled/solid-v1/solid-primitives-scheduled.json",
  "generated": false
}
```

There is nothing for `make contracts` to regenerate from, so it skips these
entries. Supplying any generator field alongside `"generated": false` is an
error rather than being ignored: a half-filled entry is someone leaving fields
out of a generated contract, and that must not pass as a deliberate
hand-authored one. Such an entry is still declared, because the manifest is the
inventory of every package a dialect models — see [The manifest is the complete
inventory](#the-manifest-is-the-complete-inventory). `node scripts/dialect-manifests.mjs validate` — part
of the universal check set — enforces all of that and fails on any declared
artifact that does not exist, so a half-added package cannot ship as a dialect
that silently models nothing.

`packagePathEnv` exists because this repository has no root `package.json` and
therefore no `node_modules` to read the audited releases from. Generation and
drift checks read a package path from that variable, falling back to
`defaultPackagePath`. Point each one at an installation of the exact pinned
version:

```sh
mkdir -p /tmp/contract-packages && cd /tmp/contract-packages
npm init -y >/dev/null
npm install --ignore-scripts --no-audit --no-fund \
  solid-js-1x@npm:solid-js@1.9.14 solid-js@2.0.0-rc.0 @solidjs/web@2.0.0-rc.0
```

```sh
SOLID_V1_SOLID_JS_PACKAGE=/tmp/contract-packages/node_modules/solid-js-1x \
SOLID_V2_SOLID_JS_PACKAGE=/tmp/contract-packages/node_modules/solid-js \
SOLID_V2_SOLIDJS_WEB_PACKAGE=/tmp/contract-packages/node_modules/@solidjs/web \
  make contracts-check
```

### Steps

1. **Declare it** in `rust/dialects/<id>/dialect.json`, as above.
2. **Teach the generator its semantics.** Add the `generatorTarget` and its
   reviewed callback/return tables to `solid-contract-gen`
   (`rust/crates/solid-facts-backend/src/bin/solid-contract-gen.rs`). Read the
   runtime implementation for each claim; a signature does not carry it.
3. **Generate** with `make contracts`, which writes the review contract and the
   Rust export index for every declared package.
4. **Register the export index** in `rust/crates/solid-dialect/src/exports/mod.rs`
   and consume it from the vocabulary implementation.
5. **Produce the bundled runtime contract** at the declared `bundledContract`
   path, and decode it in `diagnostics.rs`. Its evidence URI must be
   `bundled://<id>/<package-slug>.json`, matching the artifact path.
6. **Verify** with `make contracts-check` and `make contract-conformance`.

For a whole new dialect rather than one package, follow
[adding-a-dialect.md](adding-a-dialect.md), which wraps these steps in the
vocabulary, compiler, catalog, and detection work.

### Runtime probes and the lock

Set `probeRuntime` when the contract's claims are checked against an installed
release. `node scripts/check-bundled-contracts.mjs` then installs the exact
pinned release, checks its export surface and npm integrity, verifies every edge
in `pkg/contracts/bundled/runtime-lock.json`, and executes every declared
behavior probe in client, server, development, and production condition modes.

**Probing is grouped by dialect, one install root each.** A single shared
install cannot host them: `@solid-primitives/scheduled` peers on
`solid-js@^1.6.12` while the 2.0 contracts pin `2.0.0-rc.0`, and npm refuses the
combination. Each dialect installs its probed packages *and* its non-probed ones,
so a peer resolves to the release that dialect audits rather than whatever npm
would pick — and `runtime-lock.json` pins the transitive closure of both.

Each dialect names its probe worker in `scripts/check-bundled-contracts.mjs`.
The worker is copied into that dialect's install root and run from there, so its
bare `import "solid-js"` resolves to that dialect's release; the shared harness
in `scripts/lib/contract-probe-harness.mjs` travels with it. A worker cannot be
shared across dialects because driving a probe is version-specific: 2.0 settles
with `flush()`, and 1.x has no such function. A dialect that declares
`probeRuntime` contracts with no worker is an error, not a silent skip.

Probe identity is `(dialect, package, entrypoint, export, claim)`. The dialect
is part of it because both dialects declare `solid-js` at different versions,
and a name-only key would merge two different packages' observations.

`node scripts/check-contract-pins.mjs`, in the same target, covers what probing
cannot reach. The probe suite proves a package's identity by installing it and
reading npm's hidden lockfile, so a contract it does not install — a
hand-authored overlay, or a dialect whose runtime is not probed — would be
pinned by a version string alone. A version string is not a pin: republished or
mutated contents keep the version, and the contract would still claim to
describe them. So every bundled contract records the integrity of the exact
tarball it was audited against, that integrity is checked against the registry,
and a contract recording none fails.

`--write` records passing modes as `probed` row evidence on claims that already
exist. **It does not repair a lock or probe mismatch, and must not be taught
to.** A probe failure means the package does not behave the way the contract
says; a lock mismatch means the package that was probed is not the package that
was audited. Neither is drift in a derived artifact, and neither is fixed by
regenerating.

### Composed artifacts

When a bundled artifact is assembled from checked-in inputs rather than
generated directly from a package, declare `composeScript` and `composeInputs`.
`node scripts/dialect-manifests.mjs check-composed-contracts` runs each script
with `--check`, failing when the checked-in artifact is stale relative to its
inputs. The Solid 1.x contract works this way: it is composed from a per-subpath
export census and the reviewed semantics map.

### Version bumps

A pinned version is an audit boundary, not a dependency range. Moving one means
regenerating the artifacts *and* re-reading the claims the generator's tables
assert, because `--check` compares the artifact to those tables and has nothing
to say about whether they still describe the runtime. A newer prerelease is
reviewed, never silently substituted. Consumers see the same boundary from the
other side: an installed version other than the audited one is refused and
reported as a stale contract.

### The manifest is the complete inventory

Every gate above enumerates the contracts a manifest **declares**, which leaves
one hole they cannot see: a package a dialect models but no entry names is
covered by nothing at all. It silently has no contract, and every project
importing it reports `SC9005` forever.

`every_modeled_package_is_declared_in_the_assembly_manifest`, in
`rust/crates/solid-facts-backend/src/dialect.rs`, closes it by deriving the
expected set from the dialect instead of the manifest. A package is modeled when
the vocabulary owns one of its modules (`Dialect::modules`) or the backend
compiles a contract in for it (`Dialect::bundled_packages`); the two sets must
match the declared packages exactly. An undeclared modeled package fails, and so
does a declaration for a package the dialect neither owns nor bundles, which is
dead weight.

Module specifiers collapse to package roots first, the same way contract
discovery resolves them: `solid-js/store` and `@solidjs/web/frames` are subpaths
of one installed package, not packages of their own.

The check runs with `cargo test -p solid-facts-backend --lib`, and therefore in
`make verify`. It reads the checked-in `dialect.json` files directly, so it
fails on the manifest as committed rather than on a regenerated copy.

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
