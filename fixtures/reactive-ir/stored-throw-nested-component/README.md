# stored-throw-nested-component

**Claim (ADR 0908).** In an open program a component counts as reachable from outside only when it is itself exported (its binding,
or a top-level export's own function), never because it is written inside an exported declaration. A local component's mounts are
followed: under a boundary whose fallback takes the error, a tracked effect's plain throw (rc.21 `flush()`, no diagnostic) is the
program's own handling, so its SC3001 is uncertifiable (`NestedHandled`; pattern `flip/st-tracked-handled-flush`, Chrome logs nothing).
Violations: the same component mounted under no boundary (`NestedUnhandled`), and an exported component (`ExportedKid`), which an
outside mount may leave unhandled. The stub is `stored-throw-observation`'s plus rc.21's `flush` overloads; no `package.json`, so the
program is open.
