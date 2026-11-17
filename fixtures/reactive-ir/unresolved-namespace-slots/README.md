# unresolved-namespace-slots

**Claim (ADR 1054).** A binding of the program's own that copies a Solid export (a destructuring slot of the namespace, ADR 0459;
`const m = S.createMemo`; `const m = createMemo`; a closed holder's member, review D36) names that export only where the import it
copies resolved. Here `solid-js` holds only its `package.json`: every import is the checker's unknown symbol, so no copy names a
primitive, and no claim through one is proved. Each source is SC9012 `import-unresolved`; nothing is a violation.

Before ADR 1054 the copies were named from the module specifier alone: SC7002 at `Slot`, `Member`, `Alias`, `Held`, SC7001 at
`SlotEffect`, SC4001 at the module-scope `cleanupSlot` were violations here. `Direct` and `Namespace` (the import itself) were already
SC9012 only. `Shadowed` (a parameter named like the namespace) is silent in both.

Twin: `unresolved-namespace-slots-resolved`, the same `App.tsx` with `solid-js` resolved: every copy is a violation (Chrome rc.21,
oracle case `fixtures/runtime-oracle/unresolved-namespace-slots-resolved`).
