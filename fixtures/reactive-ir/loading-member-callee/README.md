# loading-member-callee

**Claim (ADR 1011).** A closure handed to a call is read as run where it is written when the call's target is a project function the analysis
resolves exactly, whether the callee is an identifier or a **member of a namespace import** (`H.run(cb)` with `import * as H from "./helpers"`):
the cover walk goes on through the callee's call edge to the component. `layer_cover` asked only `function_called_at`, which names an identifier
callee, so `H.run(cb)` read as "handed to a call the analysis does not resolve" and a violation became an SC5003 uncertifiable, blocker
`mount-unresolved`. Found by the audit of main 78ddeb849 (24 mx-recall programs, `module-ns` layer, lost at #234, ADR 0978).

`App.tsx` mounts every component, so no mount is unresolved; no `Loading` stands anywhere. Chrome (rc.21) warns ASYNC_OUTSIDE_LOADING_BOUNDARY at the
`slow()` read of every component that calls the closure while it renders; the snapshot pins the placement per component:

- violation at the read: `Named` and `NamedReexport` (identifier callees: the control, unchanged), `Namespace` (the claim);
- uncertifiable, no violation: `Stored` (the callee stores the closure: Chrome is quiet, nothing runs it), `Replaced` (an assignment replaced the
  literal's method), `Shadowed` (a local `H` whose method never calls the closure), `Handed` (the callee is a prop: no project function resolves);
- uncertifiable, not claimed: `Reexport` (`export * as helpers`, imported by name), `DefaultMember` (a default export's method), `LiteralMethod`:
  Chrome warns at each, and the old and the new build agree on `call-attributed-read` (no call edge places the closure in the callee).

Chrome checks of the same programs: `scripts/fuzz/patterns/membercallee/` (`ns-run` reports the code, `ns-keep` and `ns-run-covered`, the same call
inside a `Loading`, are quiet and carry no violation).
