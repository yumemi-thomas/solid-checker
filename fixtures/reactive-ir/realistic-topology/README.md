# realistic-topology

A project shaped like a project (components in separate files, helpers called from bodies, a cross-file module source) with `programBoundary: "closed"`: `Card` (`props.title`), `Badge` (destructured `label`) and `readCountNow()` in a render scope are proven violations; `Plaque` (every caller static), `watchCount()` (read sealed in a tracked callback) and its effect (called from a component body) are silent. Open-world behaviour is covered by `program-boundary-closed` and the `engine/eslint-reactivity-*` fixtures.
