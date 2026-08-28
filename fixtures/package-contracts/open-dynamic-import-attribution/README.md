# Open dynamic import attribution

`loadModule` contains a non-literal `import()` and is reachable from the public
`load` and `loadLater` functions. Their dynamic-module closure domains remain
locally open; `identity` cannot reach the load and retains its independently
proved `returns=argument[0]` leaf.

The generator may make this split only from exact local containment, references,
and export bindings. If any of those identities is ambiguous, the fixture's
entrypoint-wide refusal remains the required fallback.
