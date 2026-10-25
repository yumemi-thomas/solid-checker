# owned-write-option

ADR 0382: a setter's source may opt out of the owned-scope write guard with `ownedWrite: true`, and only an exact options literal proves the key's value.

- `plain`, `named` and `off` (`ownedWrite: false`): literal options without a true key keep the SC2001 violation in the memo compute.
- `on` (`ownedWrite: true`): the guard exempts the write (`@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js:5622`), no finding.
- `fromVariable`, `fromSpread`, `fromValue`: options the analysis cannot read may set the key, so the write is SC2001 `uncertifiable`, never a violation and never a silent clean (the same shape as SC7002's `sync`).
- `store` and `plainStore`: `createStore` and its setter never read `ownedWrite` (`dist/dev.js:5847-5944`: `storeSetter` calls `devGuardStoreSetterWrite` unconditionally, `createStore$1` ignores its options beyond `shallow`/`name`), so both stay violations. `StoreOptions` declares no `ownedWrite` (`dist/types/store/types.d.ts:30`); the variable keeps `tsc` quiet about the extra key.
