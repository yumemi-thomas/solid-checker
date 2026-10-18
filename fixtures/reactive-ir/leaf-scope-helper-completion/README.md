# leaf-scope-helper-completion

**Claim (ADR 0275).** A forbidden operation reached through a helper called in a leaf owner's callback is a violation only when the helper runs it on every normal completion, and every hop to it is likewise unconditional in its caller. An operation behind a branch, an early return, a thrown guard, a logical assignment, a loop or a possible `await` in a helper is a proof obligation: the path may depend on this call's arguments. A branch written in the leaf callback itself stays under the statically-present-code premise (`cleanup::reached_on_every_call`, `solid_facts::ast::unconditional_calls`).

| Case | Finding | Why |
| --- | --- | --- |
| `Unconditional` | `leaf-owner-forbidden-call` violation | `register` always calls `onCleanup` |
| `AfterGuardThrow` | `leaf-owner-forbidden-call` uncertifiable | a thrown guard skips the registration, and a caller may catch it |
| `NestedUnconditional` | `leaf-owner-forbidden-call` violation | both hops unconditional |
| `ArgumentBranch` | two `leaf-owner-forbidden-call` uncertifiable | the `@solid-primitives/timer` shape: the argument picks `onCleanup` or `createEffect` |
| `Guarded` | `leaf-owner-forbidden-call` uncertifiable | behind the helper's branch |
| `Looped` | `leaf-owner-forbidden-call` uncertifiable | a loop body may run zero times |
| `NestedConditional` | `leaf-owner-forbidden-call` uncertifiable | the second hop is conditional |
| `ConditionalAwait` | `leaf-owner-forbidden-call` uncertifiable | past a conditional `await` the helper may already have suspended |
| `LeafBodyBranch` | `leaf-owner-forbidden-call` violation | the branch is in the leaf callback, under the premise |
| `LogicalAssignment` | `leaf-owner-forbidden-call` uncertifiable | `\|\|=` runs its right side only when the target is falsy |
| `AfterFail` | `leaf-owner-forbidden-call` uncertifiable | a `never`-returning guard may throw into a caller that catches it (ADR 0278) |
| `AfterAssert` | `leaf-owner-forbidden-call` uncertifiable | an `asserts` guard may throw into a caller that catches it (ADR 0279) |
| `Clean` | none | no forbidden operation on any path |

`solid-js.d.ts` and `tsconfig.json` are copied from `leaf-scope-host-callbacks`.

`Immediate.tsx` exercises the same body walk for immediately invoked literal
arrows and function expressions. Clean bodies and generator bodies that never
start carry no leaf obligation. Async calls inspect their synchronous prefix,
including the operand of the first `await`, but not its continuation. Cleanup
in that prefix is a violation; cleanup after a conditional suspension remains
uncertifiable. Exact immediate literals and stable transported function targets
can use non-spread argument literals to decide their parameter branches: false
stays clean and true proves the cleanup violation. Spread arguments, unknown
calls, conditional callee selection and receiver-bearing member calls remain
uncertifiable. Parameter mutation and default rules are unchanged. The cases
are checked against published Solid RC.13 typings, without the fixture stub.
