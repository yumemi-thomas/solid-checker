# stored-throw-nested-component

**Claim (ADR 0908).** In an open program a component counts as reachable from outside only when it is itself exported (its binding,
or a top-level export's own function), never because it is written inside an exported declaration. A local component's mounts are
followed: under a boundary whose fallback takes the error, a tracked effect's plain throw (rc.21 `flush()`, no diagnostic) is the
program's own handling, so its SC3001 is uncertifiable (`NestedHandled`; pattern `flip/st-tracked-handled-flush`, Chrome logs nothing).
Violations: the same component mounted under no boundary (`NestedUnhandled`), and an exported component (`ExportedKid`), which an
outside mount may leave unhandled. The stub is `stored-throw-observation`'s plus rc.21's `flush` overloads; no `package.json`, so the
program is open.
On the census tree the two `<Errored>` elements, in exported components of this open program, also carry SC4001 `function-run-unplaced` (ADR 0883/0886: JSX that creates a boundary is an owner site, and an exported component's run is unplaced).

Since the flip of ADR 0967's default (the review of #216) this fixture's program has an open boundary (no `.solid-checker/runtime.json`), so each `flush()` written in an exported function keeps an uncertifiable row of every code its program censuses ruled out (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): an export may be entered by code this build cannot see, an action's step among it. The snapshot's additional rows are those; the claim above is unchanged.
