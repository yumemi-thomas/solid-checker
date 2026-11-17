# unresolved-namespace-slots-resolved

**Claim (ADR 1054, positive twin of `unresolved-namespace-slots`).** With `solid-js` resolved (`solid-js.d.ts`, copied from
`async-options-unreadable`, the published rc.21 declarations), each copy of a Solid export names it exactly: SC7002 at `Slot`,
`Member`, `Alias`, `Held`, `Direct`, `Namespace`; SC7001 at `SlotEffect`; SC4001 at the module-scope `cleanupSlot`. `Shadowed`
destructures a parameter, not the import: silent.

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/unresolved-namespace-slots-resolved`.
