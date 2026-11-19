# after-await-forin

**Claim (ADR 1117, the JS-semantics axis's `js-forin`).** A `for…in` walks its object's keys with no call written: a Proxy's `ownKeys` and `getOwnPropertyDescriptor` traps run project code at the loop. Written after an async memo's `await`, the loop's object is an after-await site (SC1002 uncertifiable, `read-after-await-unaccounted`), anchored at the object, where Chrome rc.21's frame is.

| case | row |
|---|---|
| `for (const k in keyed)` after `await pause()` | a row at `keyed` |
| the same loop before the await | none |

A `for…in` over a primitive is rejected by tsc, so no primitive case is written; the site is skipped where `proven_primitive` holds.

Stubs: `solid-js.d.ts`, `node_modules/solid-js/package.json`, `package.json` and `tsconfig.json` copied from `after-await-net`.
