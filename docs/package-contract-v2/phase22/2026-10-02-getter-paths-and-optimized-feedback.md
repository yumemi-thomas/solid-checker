# Getter paths and optimized runtime feedback

This pass adds two type-valid runtime failures, four source explanations for
earlier quiet or incompletely modeled consumers, and a working attribution engine
with dependency optimization enabled. It performs **72 fresh browser executions**:
ten getter consumers, twelve automatic-channel consumers and fifty broad consumers.
Three additional candidates are rejected by published typings before execution.

The production analyzer, accepted contracts, public schema and finding snapshots
are unchanged. Source observations remain informational and uncertifiable as
package-wide behavior. Actual runtime errors are retained as execution evidence.

## Two writes that published types allow

Both consumers pass strict TypeScript against `@solid-primitives/static-store`
at its retained published version:

```tsx
const [state] = createStaticStore({ count: 0 });
state.count = 1;

const derived = createDerivedStaticStore(() => ({ count: count() }));
derived.count = 1;
```

Each event-time assignment throws:

```text
TypeError: Cannot set property count of #<Object> which has only a getter
```

Both errors map to the original consumer assignment. There is no Solid diagnostic
code for this JavaScript exception; a feedback system must retain ordinary runtime
errors as well as the diagnostic subscription. The valid controls use the static
store setter or update the derived store's input signal. Both update correctly.
The publisher's signatures return writable `T` / `Next`; making those types
read-only would prevent this particular mistake earlier.

Two superficially similar candidates belong to TypeScript:

```text
TS2540: Cannot assign to 'width' because it is a read-only property.
```

`createElementBounds` and `createElementSize` publish read-only return types.
Their assignment candidates never execute and add no checker finding. The earlier
async-setter channel candidate is also excluded with TS2345. Package behavior
cannot justify duplicating these published typing errors.

The controls expose a precision trap. A static store's getter can be replaced
with a writable data descriptor before the assignment, and that consumer is valid.
The plain `getElementBounds` result is also writable and accepts the assignment.
The source pass stays quiet on both. A returned object's original getter is not
proof that all later writes must fail.

## Finite getter source paths

`GetterPaths` extends the frozen source experiment without changing its extractor
or lowerer. It supports finite object keys, literal getters, finite `for…in`,
`Object.keys(...).forEach(...)`, and getter installation through the exact global
`Object.defineProperty` declaration. Standard intrinsic behavior is an explicit
assumption; this is not a package contract or a negative closure proof.

Each admitted field records a specific path, an exact signal/memo creation call,
and its later accessor invocation. Every interpreted returned alternative must
contain that field and a positive producer path. Branches with a plain getter,
different keys, shadowed core imports, shadowed `Object`, unknown keys, prototype
initializers, unresolved loops and exhausted budgets stay open. There is no
wildcard trust for all fields of an object.

The same earlier 50-consumer corpus now receives four additional source notes:

| Consumer | Source return path |
| --- | --- |
| Static store captured member | `[0, "count"]` through its lazy signal getter |
| Derived static store captured member | `["count"]` through a memo getter |
| Element size captured width | `["width"]` through its dependency's static store |
| Element bounds captured width | `["width"]` through its installed memo getter |

The intentional static-store snapshot also receives a note. These are **source
explanations, not automatic defect warnings**. The existing **4/23** static
detection count is unchanged; these notes are a separate category. The new
namespace-imported snapshot receives a note, with its live control quiet. A
snapshot read inside a known memo compute, including a parenthesized callback,
stays quiet. No new valid-control source note appears among the ten new executed
getter specimens.

The mouse case remains open: getter-bearing spreads, unresolved fields,
optional/spread calls and the depth budget prevent a usable return path. A local
seed alias also remains unresolved by this bounded argument interpreter. Proxies,
reflection, arbitrary captured mutation, future descriptor changes, async phase
and full package behavior are not proved. The original app-modal factory wrapper
also remains open.

### Concrete arguments make a large difference

An inventory of 97 retained roots inspects **530 source-resolved function exports**
with every parameter unresolved. Only two exports expose admitted finite getter
paths in that profile: element size and window size, both in resize-observer.
This is not an estimate of consumer misuse coverage or all published functions.
Initializer-produced exports and other unsupported shapes are not silently added.

Concrete literal and callback argument shapes admit the static and derived store
paths used by the real consumers. Required input facts therefore matter even
when no accepted contract is involved. Three roots remain refused for missing
runtime entries: animation, controlled-props and virtual.

The inventory takes 3.8 seconds on this machine. The ten new consumer source
passes take about 83 ms median after their TypeScript programs are built. This
includes local closure checks and standard-library symbol resolution; it excludes
browser and native analysis and is not an editor latency benchmark.

## Attribution with dependency optimization enabled

The earlier optional engine could silently use a different copy of internal
state when only the core was optimized. This profile places the public Solid,
web and attribution entries into the same optimization run. Direct signals
entries are also requested where available. The engine attaches to the actual
app core, confirmed by the live capability probe and recorded graph runs.

The verifier follows actual import/export edges from optimized files, records
their digests and optimization metadata, and finds shared reachable chunks.
It validates **60 optimized graphs**: twelve channel consumers and 48 rc.9 broad
consumers. Four pnpm dialog consumers expose signals only transitively, so their
public entries reach the inlined signals code through shared chunks rather than
a separate optimized signals root. A version number or requested option alone
does not prove that the engine is live.

The twelve channel executions retain all five target feedback cases, with seven
quiet controls and the same observed values and error messages. The fifty-case
replay retains **15/24** directly detected targets and **26 quiet controls**.
No additional advisory code appears in that broader replay. The two query
consumers use rc.4; they execute with the attribution capability absent and are
explicitly refused by the rc.9 source vocabulary.

One dialog-trigger failure produces two Playwright error records instead of the
earlier one. The unique exception and observed values agree, but full error
delivery parity fails. The verifier records this difference rather than hiding
it behind message deduplication. The earlier focused error-identity study remains
relevant; a production UI needs a deliberate error transport policy.

This is a tested optimized development profile, not a completed default plugin
integration. Original-file guard/origin traces remain separate from this profile.
The official `@solidjs/diagnostics` bridge artifact is still unavailable locally;
its integration, HMR lifetime, SSR/hydration and other bundlers remain unmeasured.

## Evidence and verification

Isolated implementation: `getter-paths.mjs`, its eight tests, `getter-path-cases.mjs`,
`getter-path-study.mjs`, `getter-path-inventory.mjs`, and
`validate-getter-and-prebundle.mjs` under `benchmarks/reviewed-package-models/`.
The existing browser runner adds an explicit optimized-attribution option and
records whether the capability was actually used.

Ignored evidence under `rust/target/`:

- `getter-path-browser/results.json`;
- `getter-path-study-final.json` and `getter-path-inventory.json`;
- `automatic-attribution-prebundle/results.json`;
- `automatic-breadth-prebundle/results.json`;
- `getter-and-prebundle-validated-handoff.json`.

The source study contains 62 observations, with two published typing exclusions
and two older-runtime refusals. The validator authenticates consumer/package bytes,
source producers, typing exclusions, optimized graphs, actual engine installation,
source locations and the retained delivery-parity failure. All **44 prototype
tests**, syntax checks and universal handoff checks pass. Full `make verify`,
native fixture coverage, ownership gates and contract certification are deferred
because product semantics and accepted artifacts did not change.

## What this establishes

Runtime diagnostics and ordinary exceptions provide useful automatic feedback
across package boundaries without authoring a complete package model. They can
work with optimization enabled when the observer shares the app runtime. Bounded
source analysis adds explanations before execution, including fields hidden behind
transitive helpers, but unknown arguments and intent limit its reach.

The investigation still does not prove most-package misuse coverage. A practical
system needs shared runtime hooks, precise consumer-scoped source paths, and
explicit update/lifetime expectations for quiet failures. Empty source models,
observed quiet execution and informational snapshot notes cannot replace those
missing facts.
