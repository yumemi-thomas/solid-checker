# Open dynamic import attribution

`loadModule` contains an open `import()` and is reachable from the public
`load` and `loadLater` functions. Those exports must be omitted from a generated
schema-v1 contract. `identity` cannot reach the load and retains its independently
proved `returns=argument[0]` summary.

The generator may make this split only from exact local containment, references,
and export bindings. If any of those identities is ambiguous, the fixture's
entrypoint-wide fail-closed behavior remains the required fallback.
