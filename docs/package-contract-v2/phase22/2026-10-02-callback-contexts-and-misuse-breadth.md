# Callback contexts and broader misuse feedback

The new sample establishes **116 callback write-error examples across 47
packages**, each with a clean read-only control and a diagnostic mapped to the
injected application write. Inputs are generated from actual published types,
without per-package argument recipes or casts. These observations extend the
previous construction ownership sample.

Combining the two samples gives examples of useful feedback for **166 exported
APIs across 61 of the 97 retained package roots**. This is evidence for common
misuse feedback in a majority of this sampled Solid-primitives ecosystem. The
count measures exported names, including aliases, and does not count distinct
implementation defects or establish coverage of most possible misuses. Packages
from other ecosystems remain outside this sample.

The subsequent [caller-origin audit](2026-10-02-cross-package-callbacks-and-validation-flows.md)
separates one direct application invocation of a returned callback from the
other 115 write examples. The 116-error count remains valid for the original
criterion; interpreting all of them as package-invoked callbacks would overstate
this evidence.

Two precision controls establish necessary conditions for earlier warnings:
six callbacks allow the same write from an imperative caller, and twelve allow
it with the native signal's explicit `ownedWrite: true` option. Callback context
alone cannot establish that a write is forbidden.

Everything here is experimental, with `authority: false` and `certification:
false`. Production analyzer behavior and accepted package contracts are unchanged.

## Sampling without package recipes

`callback-selection.mjs` examines all 97 retained installations, including
packages with no previous positive ownership model. It resolves exact exported
declarations and synthesizes bounded callback-bearing arguments from published
types. Every candidate goes through strict TypeScript admission. The final
instrumented consumers are checked again before execution.

| Measure | Result |
| --- | ---: |
| Exported symbols examined | 1,128 |
| Exports with synthesized callback candidates | 289 |
| Exports with an admitted callback witness | 259 |
| Packages with admitted witnesses | 77 |
| Admitted read/write pairs with typing errors | 0 |
| Pairs whose package invocation executes in both roles | 249 |
| Exports with a callback observed in the read-only role | 135 |
| Exports with a public observer present in that role | 91 |
| Exports observed only without a public observer | 44 |
| Clean read-only controls | 236 |
| Exports with an exactly mapped callback write diagnostic | 125 |
| Exports with a mapped diagnostic and a clean paired control | 116 |
| Packages with those confirmed pairs | 47 |

The generator considers up to four signatures, five arguments, bounded object
properties and three input profiles. It selects the first admitted candidate
per export. Thirty candidates have no admitted witness. Animation's published
root declarations do not resolve. Other exports may have no callback argument,
be non-callable, or exceed the bounds; their omission makes no safety claim.

The consumer owns its signal and package invocation through `createRoot`.
Callback instrumentation records the callback's identity, public owner and
observer, then reads the signal. The write variant additionally calls the exact
captured signal setter. Synthesized callback return expressions, including
nested returned functions, stay intact.

The flow consumes only a return callable whose actual inferred signature allows
zero arguments, or such a callable in tuple position zero. It changes the signal,
waits briefly, and disposes the root. It does not invoke arbitrary returned
methods or synthesize external events, network services, user permissions or
component mounting. Eighty confirmed pairs observe a callback again during the
change stage; that stage marker is an execution observation, not a full proof of
dependency causality.

## What counts as a callback write error

The bridge observes Solid rc.9's actual `REACTIVE_WRITE_IN_OWNED_SCOPE` diagnostic.
The validator requires its original source location to fall inside the exact
application setter call in a tagged callback. Lexical symbol resolution excludes
shadowed setters. A diagnostic emitted elsewhere in the package cannot satisfy
that check.

The read and write consumers must have identical tokens after removing just
those callback setter statements. The read control must execute without errors,
page errors, console errors, execution diagnostics, blocked requests or saturated
instrumentation. A callback marker must also exist for the diagnosed write.
Two exports have additional write diagnostics that cannot be mapped to injected
callback writes; those diagnoses do not enter the confirmed count.

Package closures, resolved declarations, actual runtime versions and consumer
source hashes are checked. Each browser study freezes the local source closure
and retains its bytes; before and after inputs match. The selector's source
closure is now included in that freeze. This is evidence provenance for a finite
experiment, without certification authority.

The primary four studies contain 518 consumer records. One write observation
failed before browser launch because no ephemeral port was available. A fresh
two-consumer replay replaces that explicitly unavailable pair, while retaining
its original history. Successful observations cannot be overwritten by retries.
Caller and permission controls add 12 and 24 consumer records, respectively.

An earlier instrumentation implementation produced malformed source when a
TypeScript printer mixed nodes from separate source files. All 160 specimens in
that run were rejected by TypeScript before execution and are excluded. Text
insertions now preserve expression bodies, with a nested-function regression.
An earlier selector version also refused the destructure root through an
undefined tuple element; the guarded final selector admits its published types.
Incomplete and excluded outputs remain separate from the validated studies.

## Caller scope changes the answer

Six APIs are selected automatically from confirmed pairs whose bounded source
paths invoke the callback in the caller's context. Their signal stays owned,
while package invocation and return consumption move to an imperative caller.
Arguments and callback implementations stay identical.

| API | Owned caller | Imperative caller |
| --- | --- | --- |
| `event-bus.once` | Callback write error | Write succeeds |
| `mutable.modifyMutable` | Callback write error | Write succeeds |
| `range.mapRange` | Callback write error | Write succeeds; separate cleanup warning |
| `scheduled.createScheduled` | Callback write error | Write succeeds |
| `utils.withAccess` | Callback write error | Write succeeds |
| `utils.wrapSetter` | Callback write error | Write succeeds |

Every previously implicated callback actually executes with no public owner or
observer in the imperative observation. Both roles pass published typing and
have no invocation errors. Five complete imperative controls are clean.
`mapRange` retains `NO_OWNER_CLEANUP`, which is a different ownership claim.

Thirty-one confirmed broad-sample pairs observe their read-only callbacks only
without a public observer. A missing observer therefore cannot be treated as
permission to write: ownership and execution phase still matter. Conversely,
source knowledge that an API invokes a callback does not establish the caller's
ownership. A package-wide callback prohibition would misdiagnose these controls.

## Explicit permission also changes the answer

The next control selects the twelve confirmed APIs with observed source
assumptions for tracked callback contexts. It preserves package invocation and
callback bodies, and changes only the probe signal to
`createSignal(1, { ownedWrite: true })`.

All twelve APIs across nine packages execute the previously diagnosed callbacks
successfully, with an owner present and no feedback or errors in the write role.
Their read-only controls also remain clean. Real published types admit the option.
These examples include cursor, event-listener, focus, form, memo, pagination,
range, signal-builders and video packages.

An earlier source warning must resolve the write target and its configuration,
as well as the callback's execution context. Unresolved targets or options stay
uncertifiable. The experiment does not add an unconditional static callback
write rule.

## Source extraction currently reaches a smaller slice

`CallbackPaths` follows exact installed source references and records positive
context assumptions for bounded paths. It distinguishes tracked compute, effect
apply, settled apply, explicit untracking, root setup and an unresolved caller.
Async/generator helpers, stored callback bodies, unknown conditional invocation
and shadowed native primitives stay open. It does not claim callback cardinality
or that registered callbacks will necessarily execute.

Twenty exports have retained source assumptions; 95 have raw footprints. Only
18 of the 116 confirmed write pairs have any retained source assumption. The
browser catches the other 98 through shared runtime behavior. Fourteen source
callback identities across thirteen exports are observed with a public observer;
twelve of those exports have clean paired write-error witnesses. The observation
does not prove that every invocation has that context. `getObserver()` is also
an observation of public runtime state, without a universal dependency-tracking
guarantee.

This makes runtime diagnostics the broadest measured layer for this misuse
family. Source extraction can supply earlier conditional warnings on the paths
it actually resolves. Extending the source walker remains useful, but the current
source assumptions cannot substitute for the measured runtime breadth.

## Outcomes that stay unverified

Ten admitted APIs fail to execute their consumer module: controlled-props has
one, keyed six, share one and virtual two. Cached runtime-entry or dependency
resolution failures are retained. No package install or network repair is used
to reinterpret those outcomes.

Nine exports have mapped callback write diagnostics but lack a clean read
control: clipboard lacks permission; geolocation is denied; visibility observer
raises `setterFn is not a function`; sorting rejects a generated comparator
result; two SSE controls lack an endpoint; context and tween report a strict-read
diagnostic; countdown already reports an internal write error. These outcomes
do not support a new application-misuse claim about the injected write.

Another 114 successfully invoked APIs never call a generated callback during
the read-only flow. That is a coverage gap, not proof that the callback cannot
run or that its body is safe. Returned methods, real component props, external
events, broader input domains and delayed callbacks need additional flows.

All results use the retained Solid rc.9 artifacts and one cached Chromium/Vite
development profile. SSR, hydration, production builds, HMR, different bundlers,
multiple runtime copies, arbitrary async ownership and other package ecosystems
remain outside this evidence. Intent-dependent stale snapshots and background
lifetimes still need explicit application expectations, as earlier studies show.

## Practical direction

The measured route is to combine shared development runtime feedback, source
analysis of resolvable package paths, and explicit expectations for behaviors
whose correctness depends on intent. Published types can generate regression
consumers for many packages with little manual authoring. Runtime reports should
point to the actual application operation and preserve the package execution
path. Static violations still require the project's semantic proof standard;
source assumptions and quiet traces must keep their limited status.

This gives a credible way to catch common ownership and phase mistakes broadly
without certifying every package API. Package-specific protocols, domains and
unexecuted paths still require additional evidence. The experiment supports
continued implementation work; it does not establish complete package coverage.

## Verification and local evidence

- Seventy-six experimental unit tests pass, including exact callback contexts,
  open timing paths, nested expression preservation and setter shadowing.
- The primary, caller and permission studies pass their frozen-input, real-type,
  package identity and exact diagnostic-attribution validators.
- `make verify-fast`, schema JSON, dialect manifests, source syntax and whitespace
  checks pass. Full verification, coverage, ownership gates and certification
  probes are deferred for this isolated research change. Production analyzer,
  contracts, schema, manifests and finding snapshots did not change.

Local evidence includes `rust/target/callback-selected-v3.json`, its catalog,
`callback-validated-final.json`, `callback-browser-fixed-{0,1,2,3}`,
`callback-browser-port-retry-2`, `callback-imperative-validated.json`,
`callback-callers-validated.json`, `callback-permitted-validated.json`, and
`callback-permission-controls.json`. The local outputs are ignored build
artifacts; the experimental generators and validators are retained in
`benchmarks/reviewed-package-models/` for fresh reproduction.
