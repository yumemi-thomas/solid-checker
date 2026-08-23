# Legacy CJS entrypoint

Pins the negative control for legacy entrypoint discovery. A `main` target
without ESM package semantics remains unsupported rather than being analyzed
as though it were an ESM runtime artifact.
