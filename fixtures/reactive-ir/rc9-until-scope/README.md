# rc9-until-scope

SC2005 `no-await-primitive-in-tracked-scope`. `until<T>(fn, options?)` is new in `@solidjs/signals@2.0.0-rc.9` (`dist/types/signals.d.ts:608`, re-exported from `solid-js`); its dev body throws when `getObserver()` is set (`dist/dev.js:2717-2785`), then runs `fn` as the tracked compute of a user effect under a fresh root. The dialect answers it as `resolve` (argument 0, `Deferred`, `Creates`) and the rule mirrors the dev throw with `no-await-primitive-in-tracked-scope`'s scope proof. Against rc.3 the import is TS2305 (`release-triple-until-rc3`).
