# Explicit lifetimes and shared resource feedback

## Result

A package-independent browser resource monitor detects **11 of 12 authored
lifetime targets across seven packages**, under an explicit expectation that
their resources stop when the component ends. All **21 controls receive no new
lifetime warning**. Every consumer passes strict checking against its retained,
published package typings.

The remaining target schedules work after `await` without re-entering the
declared scope. It runs after disposal and receives no warning. The experiment
keeps that miss in its denominator. Two separate continuation specimens show
that explicitly entering the scope after `await` detects both a pending callback
at disposal and a resource created after disposal.

This adds a practical option to the earlier layered approach: one lifetime
expectation and shared browser hooks can catch resource mistakes without a
package contract or a separate cleanup recipe for each export. It does **not**
infer lifetime intent, establish universal package coverage, or create an accepted
contract. The warnings are conditional on the declared expectation.

## What was executed

The packages are the retained published artifacts on `solid-js`,
`@solidjs/signals`, and `@solidjs/web` **2.0.0-rc.9**. No installation or package
source modification was needed.

| Package | Version | Tested lifetime mistake |
| --- | --- | --- |
| `@solid-primitives/scheduled` | `2.0.0-next.2` | Debounce, throttle and idle scheduling in an event; work after an async continuation |
| `@solid-primitives/timer` | `1.4.5-next.1` | Discarding the manual interval cleanup |
| `@solid-primitives/event-listener` | `3.0.0-next.5` | Registering a component listener in an event with no cleanup |
| `@solid-primitives/resize-observer` | `4.0.0-next.3` | Re-observing with an escaped helper after its owner was disposed |
| `@solid-primitives/intersection-observer` | `3.0.0-next.3` | Restarting the observer after its owner was disposed |
| `@solid-primitives/mutation-observer` | `3.0.0-next.2` | Restarting the observer after its owner was disposed |
| `@solid-primitives/raf` | `4.0.0-next.2` | Restarting the animation loop after its owner was disposed |

There are **33 consumers: 12 targets and 21 controls**. The controls include
normal automatic cleanup, manual cancellation, an already completed timeout,
cross-kind timer cancellation, intentional background work, listener option
getters, once listeners, signal-based cancellation, and unrelated background
work while a scoped callback awaits, fractional timer ID cancellation and
object-based cancellation conversions.

Thirty-one original executions and 31 first monitored executions established
the initial comparison. The RAF case exposed repeated warnings and lost caller
locations in subsequent frames. After fixing causal propagation, all 31
monitored consumers were executed again with experiment inputs captured before
and after launch. A final precision review then added two original coercion
controls and a 33-consumer monitored replay: **128 fresh browser executions in
total**. The final comparison uses the original 33 and final monitored 33.

All final consumers have zero published typing errors, zero caught exceptions,
zero page/window errors, no blocked external requests, one disposal, and no
harness failure. The final validator checks consumer bytes, installed package
closure pins, runtime versions, behavior and diagnostic comparisons. It also
checks 33 optimized import graphs and live attribution installation. The 18
captured local experiment inputs remained unchanged during the final launch.

## The explicit boundary

An illustrative application boundary is:

```tsx
const scope = audit.scope("search component");
onCleanup(() => queueMicrotask(() => scope.end()));

const search = scope.bind(() => {
  const scheduled = debounce(sendSearch, 120);
  scheduled();
});
```

The boundary declares that browser resources started in this work should stop
when the component ends. It does not restore Solid's owner, register a missing
package cleanup, cancel anything, or repair the application. The microtask
allows the owner's synchronous cleanup callbacks to finish before inspecting
remaining resources.

In the target, debounce is created in an event with no Solid owner. The package's
owner guard skips automatic cleanup; its native timeout remains active at scope
end. The monitor emits `RESOURCE_OUTLIVES_DECLARED_SCOPE`, and the callback still
runs. In the control, debounce is created during component setup, its cleanup
cancels the same kind of timer, and the monitor stays quiet.

An explicit `background` expectation allows a resource to survive. A callback
outside a declared scope is not silently assigned to a component. Intent is
provided by the boundary, rather than reconstructed from an absent owner.

For a Promise continuation, the scoped callback must re-enter the boundary:

```tsx
const search = scope.bind(async () => {
  await ready;
  scope.run(() => debounce(sendSearch, 120)());
});
```

Without `scope.run` after `await`, the timer is missed. Holding a shared global
scope across an unresolved Promise would assign unrelated work to that scope;
the concurrent background control checks that the implementation avoids this.
Automatic compiler propagation of such boundaries is not implemented.

## Shared browser hooks

`lifetime-audit.mjs` watches timeout/interval scheduling and cancellation,
animation frames, idle callbacks, persistent event listeners, and
Resize/Intersection/MutationObserver observation and disconnection. It contains
no package export names, consumer labels or per-package models.

The implementation preserves native timer handles, callback receiver/arguments
and return, and thrown error identity. Timeout and interval cancellation use the
same handle pool. Observer constructors and callbacks keep their identities;
only observation/disconnection methods are intercepted. Event listeners retain
their callback identities, duplicate semantics and capture matching. Native
option reads are forwarded through a Proxy with the original receiver.

These are measured implementation properties, not a proof of transparent
instrumentation for every program. Once and AbortSignal listeners remain open:
the monitor records a gap and does not claim that such a listener is still
active. The option getter control observes exactly the same reads and receiver
as the original browser execution: capture twice across addition/removal, and
once/passive/signal once during addition.

Native cancellation may convert a fractional or object handle into a recorded
timer ID, or an object into a recorded event type. A missing registry key is not
evidence that a resource survived. These paths discard the affected registry
premises and record explicit gaps, without repeating the native conversion.
The new controls retain the original conversion count and execution behavior.

Timer callbacks carry the resource's original registration through subsequent
scheduled work. The initial RAF monitor emitted twelve lifetime warnings, with
later ones lacking an app location. The final monitor emits one warning at the
escaped restart call and retains the actual later operation frames separately.
It groups repeated resources by declared scope, resource kind and original call.
This grouping is for feedback delivery, not a claim that all browser errors are
the same exception.

All eleven final lifetime warnings map to original consumer locations. No new
Solid semantic diagnostic is introduced by the monitor. RAF's initial startup
read produces a shared `STRICT_READ_UNTRACKED` warning in both target and control,
with and without monitoring. It remains in the results and is not counted as an
introduced lifetime defect. Consequently “21 quiet controls” applies to the
**new lifetime channel**, not to all raw Solid diagnostics.

## How far the shared hooks might reach

A separate source inventory walks the browser root entry and its local static
runtime imports for 97 retained primitive packages. It finds **154 references in
37 package roots** whose exact symbol declarations belong to the real TypeScript
`lib.dom.d.ts`. Local/shadowed or mixed-declaration symbols confer no reference.
The validator resolves each recorded call again and checks its source and
declaration identity.

This includes timer, frame and observer references in packages such as bounds,
focus, lifecycle, presence, spring, storage, styles, tween and websocket. It is
**potential source reach**, not 37 packages with proven misuse coverage. The
inventory includes cancellation and constructor references as well as resource
creation. It does not prove that an export executes any given site. Dependency
helpers, unresolved receivers, dynamic imports and other resource families are
outside this direct-reference count.

Three root entries remain refused: animation, controlled-props and virtual.
Their missing runtime entries are not interpreted as an absence of resources.

## Cost

One local warm browser microbenchmark repeatedly registers/removes a package
event listener and schedules/cancels a timeout. After a warm-up, nine samples
each perform 800 platform operations, including 400 resource registrations.

| Profile | Median for one sample |
| --- | ---: |
| Original | 0.20 ms |
| Monitored | 2.90 ms |

The difference is about **6.75 microseconds per registration** in this run.
Stack collection on every scoped registration has a visible cost. These numbers
are not editor latency, application overhead or a portable benchmark. Production
use, default installation, sampling strategies and large-app overhead remain
unmeasured.

## What remains impossible or open

- **Undeclared intent:** a background task and a forgotten component task can
  have identical executable code. A remaining native handle alone cannot decide
  which lifetime was desired.
- **Unexecuted paths:** the monitor observes only resources that actually run.
- **Promise continuations:** the unannotated continuation target remains missed.
- **Other callback propagation:** event and observer callbacks keep native
  identity; their later resource creations are not automatically scoped.
- **Opaque resource behavior:** once/signal listeners, string timer callbacks,
  non-string event types and coercible cancellation handles remain explicit
  gaps. Pre-captured native functions, replaced platform implementations and
  multiple browser realms need further work. The finite tests do not establish
  transparency for these forms.
- **Other resources:** fetch/abort intent, streams, sockets, workers, event-bus
  subscriptions, GPU/audio resources and package-private registries are not
  watched by these hooks.
- **Boundary semantics:** shared resources, transfer between owners, delayed
  cleanup, repeated mounts, HMR, SSR/hydration and background suspension remain
  unmeasured. Inspection after a cleanup microtask is not a proof of immediate
  cancellation for every queued callback.

The result supports a development lifetime channel alongside core diagnostics,
ordinary exceptions, source explanations and behavior assertions. It does not
justify replacing all package contracts with browser hooks, or promoting
ownerless resources into unconditional violations.

## Artifacts and verification

New experiment files live under `benchmarks/reviewed-package-models/lifetime-*`.
The implementation, ten focused tests, four consumer groups, final input
capture, platform inventory and validation are separate files. The existing
browser harness and production analyzer are unchanged.

Local generated evidence is under `rust/target/lifetime-*`, especially:

- `lifetime-final-coercion/browser/results.json` and its before/after input records;
- `lifetime-platform-inventory-final.json`;
- `lifetime-validated-final.json`;
- the original comparison groups and the first RAF monitored result, which
  retain the repeated-warning discovery.

All **54 prototype tests pass**, including the ten new lifetime tests. The
universal handoff checks pass: pinned `make verify-fast` supplies formatting and
workspace Clippy; `git diff --check`, schema JSON parsing and dialect manifest
validation pass separately. Syntax checks cover all 65 experimental modules.
Full
`make verify`, contract certification, fixture coverage and ownership gates are
deferred for this isolated experiment. No production rule, schema, contract,
fixture or snapshot was changed.
