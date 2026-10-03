# Cross-package callbacks and returned validation flows

This round establishes **22 additional package-invoked callback write-error
examples across six packages** outside Solid-primitives: Floating UI, TanStack
virtual-core, Neverthrow, Seroval, Valibot and Zod. Every counted example passes
published typing, has a clean read-only control and maps the diagnostic to the
exact injected application write. Its immediate caller belongs to authenticated
package source bytes.

Seventeen of those examples require invoking a returned validator. Constructing
the object alone never calls the supplied callback. A shared driver for public
validation interfaces reaches these paths without per-package argument recipes.
All seventeen also permit the same writes with the native signal's explicit
`ownedWrite: true` option, including the async validator examples.

An audit refines the previous breadth count: one earlier error and three new
ones occur when the application directly invokes a returned callback. They remain
valid write errors, but cannot establish package callback invocation behavior.
Excluding these four from that interpretation leaves **187 exported API names
across 67 sampled packages** with ownership or package-dependent callback feedback.
Aliases still count as names. This combines different finite samples and does
not measure distinct defects, most application imports or all possible misuses.

The work remains experimental, with no certification authority and no production
rule or accepted contract changes.

## Population and execution environment

A declared population of 22 package names covers several library families.
Seventeen have usable cached installations in the retained apps. The inventory
selects the first installation whose actual dependency closure has no Solid
runtime version other than rc.9. It records other candidates and their refusals.
Five requested names have no directly declared cached installation: eventemitter3,
lodash-es, mitt, p-limit and ts-pattern. This is a selected population, not a
random or representative sample of npm.

Fresh consumer directories link to exact cached package roots, their existing
type namespaces where available, and the audited rc.9 runtime. Original package
files and retained `node_modules` are unchanged. There are no package installs
or network repairs. Vite deduplicates the native runtime graph and explicitly
prebundles the consumer's exact imports, including CommonJS inputs. Source
instrumentation does not modify package implementations.

| Package | Selected version | Admitted callback APIs |
| --- | --- | ---: |
| `@floating-ui/dom` | 1.8.0 | 10 |
| `@solidjs/router` | 2.0.0-next.26 | 10 |
| `@tanstack/query-core` | 5.101.4 | 4 |
| `@tanstack/virtual-core` | 3.17.8 | 2 |
| `neverthrow` | 8.2.0 | 1 |
| `rxjs` | 7.8.2 | 64 |
| `seroval` | 1.6.7 | 5 |
| `solid-relay` | 1.0.0-beta.29+jandibat.9dfb839 | 4 |
| `valibot` | 1.4.2 | 75 |
| `zod` | 4.4.3 | 87 |

Other inventoried roots are meta, clsx, fflate, lodash, nanoid, tailwind-merge
and sparkstone's validation package. The last root's published declarations do
not resolve. The others contribute no admitted callback witness under the
current selector. This does not establish that their APIs have no callbacks.

## The construction flow reaches a small fraction

The unchanged published-type generator examines 1,929 exported symbols and
admits 262 callback-bearing APIs across ten packages. All 524 final consumers
pass actual typing and execute their package invocation. Five further candidates
have no admitted witness: query-core's error predicate, an RxJS key comparator,
and three Valibot guard/check APIs.

| Construction/returned-call measure | Result |
| --- | ---: |
| Admitted API pairs | 262 |
| Clean read-only controls | 178 |
| APIs with a callback observed | 9 |
| Exactly mapped write-error pairs with clean controls | 8 |
| Those pairs with a package as immediate callback caller | 5 |
| Those pairs with the application as immediate caller | 3 |

The five package-invoked examples are `autoUpdate`, virtual-core's `memo`,
`fromThrowable`, `crossSerializeStream` and `toCrossJSONStream`. Their caller
contexts have an owner and no public observer. Shared Solid diagnostics catch
the application write even though these libraries need no Solid dependency or
package contract for that behavior.

Query-core's `replaceEqualDeep`, virtual-core's `notUndefined` and Seroval's
`createReference` return a generated callback that the application subsequently
invokes. That immediate application call triggers the guard. It is useful
feedback, but is excluded from the package-invoked count.

`defaultScheduler` supplies a negative observation: its callback executes with
no public owner or observer, and the write succeeds without feedback. No API-wide
callback prohibition is inferred from the other examples.

Another 253 invoked APIs never call a generated callback in the read-only flow.
RxJS contributes 64 typed constructions but no invoked callback. Operators and
cold observables need a producer/subscription flow, which this experiment does
not supply. Quiet construction is a substantial coverage gap.

## A shared validation flow adds seventeen examples

The next selector inspects the actual inferred return types for public `parse`
or `~standard.validate` members. Seventy-four returns expose a typed validation
flow. Fifty-three are excluded because their earlier construction controls fail.
The remaining 21 APIs span Valibot and Zod.
This is an adaptive discovery sample from the same retained inputs, without an
independent holdout or a measured false-positive rate for real applications.

The consumer prefers the result-returning shared validation interface where
available, preserves its receiver, and passes the finite input `1`. Public
member declaration hashes are checked; final TypeScript admission checks the
complete member invocation. Member names choose an explicit test flow and make
no static semantic claim about what the member does.

| Validation-flow measure | Result |
| --- | ---: |
| API pairs executed | 21 |
| Published typing errors | 0 |
| Clean read-only controls | 21 |
| APIs with callbacks observed and exactly mapped write errors | 17 |
| Valibot examples | 14 |
| Zod examples | 3 |
| Immediate callers confirmed in package source | 17 |

The Valibot examples include custom checks and error-message callbacks for tuple,
intersection, union and variant validation, with synchronous and asynchronous
forms. The Zod examples are transform, custom validation and stringbool's error
callback. Some inputs produce ordinary validation issues in the read-only role;
the validator returns those results without an execution error. An invalid
value presented to a validator is not itself a checker violation.

The callbacks remain unobserved for Valibot's literal and picklist and Zod's
stringFormat and json under this input. Other inputs and validation branches
remain untested.

All seventeen positive flows are repeated with the signal's explicit
`ownedWrite: true` option. Their callbacks actually run with an owner, and all
read/write roles remain free of feedback and execution errors. These controls
add 34 consumer records to the 42 validation records and 524 initial records:
**600 fresh browser observations** in this round.

## Immediate caller identity refines the earlier study too

The audit starts at an exactly source-mapped application setter call and examines
the immediate next stack frame. It does not skip missing frames to find a more
convenient caller. A source map can identify the caller's file. An unbundled Vite
filesystem URL can instead identify its exact served input module, with no claim
about its original source span. The physical file must exist within an
authenticated package/runtime graph, its URL must share the application's
origin, and its bytes are hashed.

Optimized bundle names never determine package identity. Missing frames, opaque
URLs, missing files and callers outside the graph remain unknown. Regression
tests cover both unresolved immediate frames and exact served files.

In the previous 116-example Solid-primitives study, 99 examples have a package
as immediate caller, sixteen have the native Solid runtime as caller, and one
has the application as caller (`tryOnCleanup`'s returned function). The native
runtime cases observe actual computation callback execution; they do not prove
all registration or execution paths of their package API. All 116 retain their
original valid write-error/control evidence.

Initially the audit cannot classify 99 unbundled caller frames because those
modules have no source map. Resolving their exact filesystem URLs closes this
file-identity gap. It does not reconstruct source positions or turn the evidence
into a static proof. The original strict audit is retained separately.

## Package domains and environments still matter

Eighty-four initial controls fail despite having valid published types:

- Six router APIs and three Relay APIs lack required context values. Relay's
  pagination API instead rejects the generated fragment metadata. Provider and
  GraphQL environments require richer flows.
- Seventy-four Zod constructors reject simultaneous `message` and `error`
  options. The real signature admits both fields, and the installed
  `normalizeParams` guard throws. Type admission cannot establish option-domain
  validity. These are invalid generated inputs, not diagnosed package defects.

Those exceptions are retained independently of Solid diagnostic-channel output.
Their existence shows another necessary layer: package guards can fail without
publishing a Solid diagnostic. Generic exception attribution can describe an
observed failed call; a static option warning needs an exact guard/path proof.
Caught, expected validation failures also require care before being classified
as incorrect usage.

The selector currently misses CommonJS namespace method surfaces such as Lodash,
and standard typed binary inputs needed by fflate/nanoid. Class constructors,
returned operator protocols, React/GraphQL contexts and private library protocols
remain outside bounded synthesis. Source extraction retains no callback-context
assumptions for these 262 APIs; its five raw footprints are insufficient. Earlier
static feedback for this wider population is unproven.

These observations use one rc.9/Vite/Chromium development profile. They do not
cover production builds, SSR, hydration, other bundlers, HMR, all async timing
branches, runtime-copy combinations or arbitrary package-specific protocols.

## What the result changes

Shared runtime invariants work across libraries that know nothing about Solid.
Public types and a small collection of explicit returned-object flows can test
more than construction. Source analysis can add earlier feedback where it
resolves the callback, caller phase and write target configuration. Package
guards and intent-dependent behavior remain additional sources of evidence.

The broader result supports a layered implementation with reusable flows and
precise application locations. It does not support a guarantee that every
incorrect primitive use is discoverable, or a broad static warning generated
from every callback-shaped type.

## Verification and evidence

- Seventy-eight experimental unit tests pass. Syntax checks cover 117 modules.
- Frozen case/bridge inputs, package identities, real published typing, exact
  application write locations and immediate caller identities are validated.
  Permission controls validate successful execution of the same callbacks.
- `make verify-fast` passes producer freshness, formatting and pinned workspace
  Clippy. Schema JSON, dialect manifests and whitespace checks pass.
  Full verification, coverage, ownership gates and certification probes are
  deferred for these isolated experiments.
- No accepted contracts, schemas, manifests or finding snapshots changed.

Local ignored evidence includes `cross-package-roots/run.json`,
`cross-package-selected.json`, `cross-package-validated.json`,
`cross-package-origins-origin-checked.json`, `cross-protocol-selected.json`,
`cross-protocol-clean-selected.json`, `cross-protocol-validated.json`,
`cross-protocol-origins-origin-checked.json`, `cross-protocol-permission-controls.json`,
and `callback-origins-origin-checked.json` under `rust/target/`. The four
`cross-package-browser-*` studies, `cross-protocol-browser` and
`cross-protocol-permitted-browser` retain
source copies, consumer files and browser results. Generators, drivers and
validators are retained under `benchmarks/reviewed-package-models/`.
