# namespace-solid-barrel

**Claim (ADR 0517).** A static member of a namespace import of a project module that re-exports Solid (`export * from "solid-js"`, or `export { createSignal } from "solid-js"`) is the primitive its property's symbol resolves to, exactly as a named import from the same module is. Before, `Star.createSignal(1)` named nothing and the file certified clean while rc.17 reported `STRICT_READ_UNTRACKED` and `REACTIVE_WRITE_IN_OWNED_SCOPE`.

`App.tsx`: `StarRead`, `StarWriteInMemo`, `NamedRead` are violations (`SC1001`, `SC2001`, `SC1001`); `StarTracked` (a tracked read) and `ShadowRead` (the module's own `createSignal` shadows the star's) are clean.

SC2008 through such a barrel: `rc13-flush-in-effect-callback/Barrels.ts` (it needs installed package bytes). Probes: `scripts/fuzz/probes/barrel/`.
