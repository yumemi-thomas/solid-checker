# Legacy ESM index entrypoint

Pins the unambiguous `index.js` fallback for a package whose manifest declares
ESM runtime semantics but has neither `exports` nor `main`.

Both axes land on the same last-resort branch: the case records
`legacy:index` for runtime *and* for declarations, and the deferred callback
claim on `scheduleLegacy` still comes from the runtime artifact. This is the
only fixture that pins `legacy:index` on the runtime axis --
`legacy-module-entry` pins `legacy:module`/`legacy:types`,
`legacy-module-absent` pins `legacy:main`/`legacy:types`,
`legacy-main-esm-entrypoint` pins `legacy:main`/`legacy:main`, and
`legacy-module-entrypoint` pins `legacy:module`/`legacy:index`.
