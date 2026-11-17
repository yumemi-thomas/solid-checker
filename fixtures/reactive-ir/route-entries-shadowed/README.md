# route-entries-shadowed

ADR 1025 negative (shadowed): `createRouter` is a local function that calls each `component` from a timer. No audited router
renders `Home`, so its `onCleanup` keeps `owner-callers-unenumerated`.
