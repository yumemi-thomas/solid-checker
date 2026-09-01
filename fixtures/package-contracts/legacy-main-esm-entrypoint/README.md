# Legacy ESM main entrypoint

Pins package-contract generation for an exact legacy `main` target whose
package declares ESM runtime semantics.

No `module`, no `types`, no `typings`: both axes select `main`, so the case
records `legacy:main` for runtime *and* for declarations. `legacy-module-absent`
also lands on `legacy:main` at runtime, but it declares `types` and so pairs it
with `legacy:types`; the declaration fallback all the way to `main` is pinned
only here.
