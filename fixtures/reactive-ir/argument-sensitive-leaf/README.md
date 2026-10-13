# argument-sensitive-leaf

**Claim (ADR 0280).** The leaf-scope walk decides a helper's parameter tests from the arguments of the helper call written in the leaf callback. The `@solid-primitives/timer` shape is then a proven violation for exactly the operation the delay selects, and the other operation is not reported.

| Case | Finding | Why |
| --- | --- | --- |
| `NumberDelay` | `leaf-owner-forbidden-call` violation (`onCleanup`) | `30_000` takes the `typeof delay === "number"` arm, which returns |
| `AccessorDelay` | `leaf-owner-forbidden-call` violation (`createEffect`) | a function skips that arm |
| `EitherDelay` | two `leaf-owner-forbidden-call` uncertifiable | `number \| (() => number)` decides neither arm |

`solid-js.d.ts` and `tsconfig.json` are copied from `leaf-scope-helper-completion`; `tsc --noEmit` passes.
