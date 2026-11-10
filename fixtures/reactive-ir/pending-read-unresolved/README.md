# pending-read-unresolved

ADR 0926: a pending read an SC9012 row holds gets SC5003's own uncertifiable (SC5008's too in a swallowing `try`), `pending-read-unresolved`. Positives: `ComputedKeyRead`, `BareChildAlias`, `HookMemberRead`, `HandedParameterRead`, `CaughtNestedRead`. Negatives: `SyncComputedKeyRead`, `CoveredBareChildAlias`. `Settled.tsx`: `onSettled.call` after an `await` is SC4001 uncertifiable, not in the body. Stubs from `pending-read-accounted`.
