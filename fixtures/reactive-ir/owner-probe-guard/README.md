# owner-probe-guard

**Claim.** An owner-requiring operation (`onCleanup` here; the funnel covers
every owner requirement) that runs only after the same invocation has seen a
non-null owner is not reported by `SC4001`: it is the right operand of
`getOwner() && …`, or sits in the consequent of `getOwner() ? … : …` or of
`if (getOwner()) …`, where the test is a call of Solid's own `getOwner`,
resolved by symbol through a named, namespace or aliased import
(`owners::guarded_by_owner_probe`). "No scope's disposal can trigger it" is
false on every path that reaches such a call.

Before, the owner pass judged the call by the owner context of its function
alone, so an exported helper written this way and called at module scope was
reported as calling `onCleanup` without an owner. `@solidjs/router`'s
`data/action.js` is the shape.

Measured on the published `solid-js`/`@solidjs/signals` `2.0.0-rc.9` bytes
(byte-identical to the audited archives), compiled with
`babel-preset-solid@2.0.0-rc.2` and imported under Node 24 with the `browser`
condition, dev and prod: evaluating `registry.ts` raises `NO_OWNER_CLEANUP`
exactly twice (dev), from `registerUnguarded` and `registerInverted`, and
`shadowed.ts` raises it once; the five guarded helpers raise nothing.

| Case | Finding | Why |
| --- | --- | --- |
| `getOwner() && onCleanup(…)`, `if (getOwner()) onCleanup(…)`, `getOwner() ? onCleanup(…) : …`, called at module scope | none | `onCleanup` runs only with an owner |
| the same through `Solid.getOwner()` and `import { getOwner as currentOwner }` | none | the probe is resolved by symbol |
| an unguarded `onCleanup` in the same helper shape | `SC4001` violation | runs with no owner at module scope |
| `getOwner() \|\| onCleanup(…)` | `SC4001` violation | runs exactly when there is no owner |
| a local `const getOwner = () => true` guarding `onCleanup` | `SC4001` violation | not Solid's `getOwner`: its answer proves no owner |

Not read as a guard, so these keep their requirement: a probe over a binding
(`const owner = getOwner(); owner && …`), `!getOwner() || …`, an early return
(`if (!getOwner()) return;`), and an operation after an `await` inside the
guarded region (an owner seen before an `await` is not current after it).

**Stub.** `solid-js.d.ts` holds `getOwner`, `onCleanup`, `Disposable` and
`createRoot` verbatim from the rc.9 typings, with `Owner` reduced to an empty
interface, as its header lists. Both files type-check cleanly against the stub
and against the real rc.9 installs (`tsc --noEmit`, TypeScript 5.9.3, `strict`,
`skipLibCheck` because `solid-js@2.0.0-rc.9`'s own `types/index.d.ts` fails to
resolve five of its re-exports).
