# ADR 0290: A package's owner requirement is reported at the project's call

- Status: accepted and implemented (2026-10-13).
- Owner: `owners::attribute_dependency_requirements`, run at the end of both
  owner passes; `OwnerRequirement::through_dependency`; the wording in
  `Finding::for_owner_requirement`.
- Step 1 of moving dependency analysis from contracts to source (ADR 0284).

## Context

ADR 0284 does not report a finding located in a package analyzed as source.
Most of those findings are the package's own: its component bodies, its
compiled store writes, its internal dispatch. They hold whatever the
project does, and contract mode never saw them either. Measured over the
rc.13 corpus with 11 packages, about 1,000 package-located findings were
dropped, and 36 of them were violations. Every violation but two was
package-intrinsic.

The two were readingroom's: `createTimer(…, 1500, setTimeout)` called in an
event handler after an `await`. `createTimer`'s `onCleanup` then runs without
an owner. The defect is the project's call, but the finding sat in the
package, so it was dropped. Contract mode reported it. Dropping it is a loss
of recall, and also a loss of fail-closed behaviour: the project's call could
run package code unowned with nothing said.

An owner requirement is the one kind whose truth comes from the caller. Owner
contexts propagate as bits along call edges from the project into the
package. A read in a package helper already reaches the project through
interprocedural summaries, and the leaf-owner rule already reports at the
project's call.

## Decision

After propagation, take each owner requirement located in a package that is
reported, not after an `await` in the package, and not itself attributed.
Walk back from the function that owns the operation along incoming edges
that carry the reporting bit.

- **Inside packages:** follow an edge only when the bit is inherited. An edge
  that creates the bit itself, because the package scheduled the callback,
  is the package's own context, and the walk stops there. Each hop must be a
  call written directly in the caller that enters the function.
- **At a project caller:** each call written directly in it that enters the
  package function becomes a requirement at that call. It carries the
  package operation's location in `through_dependency`.
- **Proven or obligation:** a requirement proven unowned walks only the
  proven-unowned bit and can be a violation at the call. Every other
  requirement walks the plain unowned bit and becomes an obligation.
- **ADR 0275 at the boundary:** the attributed requirement is a violation
  only when the operation is on its function's every-completion path, and so
  is each hop's call. On the first hop, the project call's own arguments may
  decide instead (ADR 0280):
  - `Unreachable`: nothing is attributed to that call;
  - `Possible`: the result is an obligation;
  - `Guaranteed`: the first hop counts as proven.

The finding is located at the project's call. Its message says the call runs
the operation inside a package analyzed as source, and its last evidence step
is the operation in the package's source.

## Consequences

- readingroom's `createTimer` cleanup is reported at the call again, as a
  violation, as in contract mode. Its accessor-delay effect branch is
  unreachable for a numeric delay, so it is not reported.
- Not attributed, and silent as before:
  - a bit seeded inside the package (its own event handlers or module
    scope);
  - a project call from module scope, which the incremental pass records as
    an edge with no source function;
  - a hop made through a callback rather than a call.
- Only owner requirements are attributed. Every other package-located
  finding is still not reported.
- No cache change. The requirements are recomputed with the owner graph,
  which already covers both files.

## Evidence

- **Fixture** `reactive-ir/dependency-source-owner` covers six calls: two
  violations, two uncertifiable, one clean and one not attributed. The two
  violations come from a two-hop numeric delay and an arrow delay. `tsc
  --noEmit` is clean.
- **Coverage:** no other snapshot moved.
- **rc.13 corpus, dependency mode** (11 packages): violations 559 → 560, the
  readingroom cleanup. Uncertifiable is unchanged at 3,556, and there were no
  errors. Base mode returns before any work.

## Review round 1

Three false-violation shapes, all fixed:
- **A member of the package function counted as a call of it**
  (`useA.reset()`, `useA.toString()`, `useA.bind(…)`). The attribution
  ignored the callee resolution's `exact` flag; it now requires it.
- **A scheduled reference blamed the direct call.** With `setTimeout(useA)`
  beside an owned `useA(f)` in the same function, the direct call was
  reported. An edge the project caller creates the bit on itself, because it
  schedules the function, attributes nothing.
- **The owner edge builder had the first flaw in base mode too.** An entity at
  a member callee's composite span names its receiver, so `projA.reset()` and
  `projA.bind(…)` gave `projA` an edge, and its `onCleanup` was taken as
  running unowned. `call_edge_symbol` gives such a call no edge, except the
  standard library's `call` and `apply`, which run their receiver.

Fixtures: `dependency-source-owner` gains `MemberInHandler` and
`ScheduledAndCalled`, both clean, and `owner-member-call-edge` pins the base
case. A `const AP = Array.prototype` alias that defeats ADR 0288's veto was
also found; ADR 0288 already places it outside the premise.

## Review round 1, second reviewer

Four false-violation shapes, all now obligations or exact:
- **The project call itself.** A violation now also needs the attributed
  call to be on its project function's every-completion path
  (`frontier_call_completes`). An aggregate unowned context is the function's,
  and a call behind a branch may run only on owned invocations. The covered
  shapes are a helper called as `start(true)` owned and `start(false)`
  unowned, an `if (!getOwner()) return;` guard, and `if (false)`. Inside the
  function, only JSX, a class or a pattern written in the function defers
  the call, so an inline JSX handler's own body is not deferred.
- **Every site counts.** When a package function enters the next through
  several calls, the first hop weighs all of them. `choice(false)` running
  the second of two cleanup sites was dropped.
- **Vite's extension order** for relative imports in a package (`.mjs`
  before `.js`; ADR 0291). A redirect now also binds an import the compiler
  resolved to another runtime file than the bundler loads.

Defects that were silently certified are now obligations:
- a project call at module scope, found by scanning;
- a package operation after an `await`, which walks every edge.

A namespace import of a selected package keeps its contract obligation,
because it is never redirected. A package name with any copy that is not
analyzed keeps its contract.

Recorded, not fixed:
- a caller's own `await` before the call is not carried on owner edges,
  which is a base-mode limitation too;
- a package hop through `runWithOwner(null, f)`;
- members of an exported object (`timer.useTicker()` from a named import).

### A `try` block is on every completion (AST facts schema 60)

Requiring the project call on its every-completion path turned readingroom's
`createTimer` cleanup, written in a `try` block after an `await`, into an
obligation. ADR 0276's completion sites treated a whole `try` statement as
conditional. Under the normal-completion premise (ADR 0279, option (b)) a
`try` block runs to its end, and only its `catch` and `finally` are
conditional. Completion sites now discount the block, and only completion
sites: straight-line calls and unconditional awaits keep the whole
statement conditional. The violation is back. Base mode is unchanged
(562/3,737). `dependency-source-owner`'s `InTryBlock` and `InCatch` pin both
sides. ADR 0283's `function h(o) { try { return o.a; } finally {} }` now
yields its Get.
