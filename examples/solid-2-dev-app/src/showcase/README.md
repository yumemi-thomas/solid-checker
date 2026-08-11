# solid-checker showcase

Each file here is intentionally written to demonstrate one capability of
`solid-checker`. None are imported by the running app, so `npm run dev` and
`npm run build` are unaffected. They are analyzed because `tsconfig.json`
includes the whole `src` directory.

Run the checker over the project to see every diagnostic:

```sh
SOLID_TYPEFACTS_BIN=../../bin/solid-typefacts \
  ../../bin/solid-checker-rust --project tsconfig.json --format json
```

| File | Diagnostic | Outcome | Why it needs solid-checker (not eslint) |
| --- | --- | --- | --- |
| `CrossFileWrite.tsx` (+ `lib/reactive-helpers.ts`) | `SC2001` reactive-write-in-owned-scope | violation | Follows an aliased `createMemo` across a re-export in another file; a per-file linter can't resolve the setter through the module boundary. |
| `Uncertifiable.tsx` | `SC4001` no-owner-effect | **uncertifiable** | An exported effect whose caller ownership can't be proven in-project. Fail-closed: neither certified nor a definite violation. eslint has no "can't prove it" verdict. |
| `AsyncLoadingBoundary.tsx` | `SC5003` async-outside-loading-boundary | violation | Uses the Solid Oxc compiler's JSX execution facts to prove an async read has no dominating `<Loading>` boundary. |
| `LookalikeAccessor.tsx` | `SC1002` reactive-read-after-await | violation | Type-aware: flags the real branded `Accessor` read after `await` but leaves an identical-looking plain function call alone — no false positive. |
| `SyncReceivedAsync.tsx` | `SC7002` sync-node-received-async | violation | Proves a `{ sync: true }` computation's callback returns a Promise — a type fact, not visible from syntax. |

Note: `AsyncLoadingBoundary.tsx` also contains a corrected variant (`ProfileOk`,
read under a `<Loading>` boundary) that is certified, and `LookalikeAccessor.tsx`
keeps the safe plain-function call to show the checker does not flag it.
