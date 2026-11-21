# effect-error-arm-read

**Claim (ADR 1163).** rc.21's `runEffect` runs a user effect's `error` member before it opens the effect's strict-read window
(`@solidjs/signals@2.0.0-rc.21` `dist/dev.js:2737-2758`, before `setStrictRead` at `:2767`). It runs from the queue's drain
(`notifyEffectStatus` queues it, `:2660-2668`). `createReaction` hands its bundle's `error` to such an effect (`:3495`). A drain
runs on the stack of whoever drains, with that stack's window (`flush`, `dist/dev-shared.js:3032-3110`). So a read written
directly in the member reads in no window when no drain the program makes can run inside one.

| project / component | outcome |
|---|---|
| `EffectErrorRead`, `ReactionErrorRead`, `NamespaceErrorRead`, `StoreErrorRead` | clean (main: SC1001 uncertifiable `function-run-unplaced`) |
| `NamedBundle` (a bundle held in a `const`), `SpreadBundle` (`{ ...base, error }`) | uncertifiable, as before |
| `variants/flush-in-root` (`flush()` in a `createRoot` body: ADR 0613's census vetoes) | uncertifiable; Chrome is quiet |
| `variants/flush-in-body` (`flush()` in the component body drains the arm inside its window) | uncertifiable; Chrome reports STRICT_READ_UNTRACKED |

Chrome: `fixtures/runtime-oracle/effect-error-arm-read`, `-flush-in-root` and `-flush-in-body`. Stubs: rc.21 versions;
`solid-js.d.ts` reduces `effect-error-arm-owner`'s and adds `createStore` and `flush`.
