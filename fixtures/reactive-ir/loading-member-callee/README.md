# loading-member-callee

**Claim (ADR 1011).** A closure handed to a call to a project function the analysis resolves exactly (an identifier callee, or a **member of a
namespace import**, `H.run(cb)` with `import * as H from "./helpers"`) is read as part of the function around the call, and the callee's body
is walked for its own boundary: where the callee calls the closure with no boundary around the call, a read in it is a violation; where the
callee (or a callee it passes the closure on to) wraps the call in a boundary, it is not.

Two defects of ADR 0978's `layer_cover` stand behind it. `function_called_at` names only an identifier callee, so `H.run(cb)` read as "handed to
a call the analysis does not resolve" and a violation became an SC5003 uncertifiable, blocker `mount-unresolved` (24 mx-recall programs of the
audit of main 78ddeb849, `module-ns` layer, lost at #234). And the closure was never looked for in the callee, so `exprArrow(cb)`, which wraps
`cb()` in `createLoadingBoundary`, left a violation where Chrome is quiet (a false positive of main, identifier callee included).

`App.tsx` mounts every component, so no mount is unresolved; no `Loading` stands in the caller. Chrome (rc.21) warns ASYNC_OUTSIDE_LOADING_BOUNDARY at
the `slow()` read of every component whose callee runs the closure with no boundary around it; the snapshot pins the placement per component:

- violation at the read: `Named` and `NamedReexport` (identifier callees: the control), `Namespace` (the claim), `Forward` (a chain of two project functions);
- uncertifiable, no violation: `NsInBoundary`, `NsInBoundaryBlock`, `IdInBoundary`, `IdInBoundaryBlock` (the callee's own boundary catches the read: Chrome is
  quiet), `ForwardBoundary` (the function the closure is passed on to has it), `Later` (`queueMicrotask(cb)`: not a call of the parameter), `Maybe`, `PassOn`,
  `Stored` (the callee stores the closure: Chrome is quiet), `Replaced` (an assignment replaced the literal's method), `Shadowed` (a local `H` whose method never
  calls the closure), `Handed` (the callee is a prop: no project function resolves);
- uncertifiable, not claimed: `Reexport` (`export * as helpers`, imported by name), `DefaultMember` (a default export's method), `LiteralMethod`: Chrome warns at
  each, and the old and the new build agree on `call-attributed-read` (no call edge places the closure in the callee).

Chrome checks of the same programs: `scripts/fuzz/patterns/membercallee/` (15 programs; the four own-boundary ones, `ns-forward-boundary`, `ns-keep`, `ns-later`
and `ns-run-covered`, the same call inside a `Loading`, are quiet and carry no violation).
