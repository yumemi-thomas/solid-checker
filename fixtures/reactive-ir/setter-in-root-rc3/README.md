# setter-in-root-rc3

A setter in a `createRoot` body throws `REACTIVE_WRITE_IN_OWNED_SCOPE` (SC2001) in dev. Store setters depend on `@solidjs/signals`: rc.1-rc.8 exempt a root (rc.3 `dist/dev.js:4111`), rc.9 does not. On rc.3, signal setters, `refresh`, `flush(...)` and memo-compute writes in a root are reported; store setters in root bodies, `ownedWrite: true`, effect apply, `onSettled` and module scope are not. A write in an `addEventListener` listener is uncertifiable. Twin `setter-in-root-rc9`. Stubs: published `@solidjs/signals@2.0.0-rc.3` `dist/types/`, JSDoc dropped.
