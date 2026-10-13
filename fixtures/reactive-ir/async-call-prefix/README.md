# async-call-prefix

**Claim (ADR 0221, G2).** An ordinary call of an async project function
initializes its parameters and runs its body on the caller's stack up to the
first possible suspension. A read written directly in that prefix, or in a
default initializer the call activates by omitting the argument, runs during
the call. It is decided per invocation, at every hop
(`interproc::body_site_runs_during_call`).

| Case | `SC1001` | Why |
| --- | --- | --- |
| `BeforeAwaitBody`, `BeforeAwaitApply` | violation | read before the first `await` |
| `OmittedDefaultApply`, `StoreDefaultApply`, `TopLevelOmittedDefault` | violation | omitted argument activates the default |
| `NestedHelperPrefix`, `PrefixThroughAsyncChild`, `DormantNestedAwait` | violation | prefix proven at every edge; a nested function's `await` does not suspend the outer one |
| `BeforeImplicitSuspension`, `PreferPrefixOrigin` | violation | no suspension precedes the read |
| `SuspensionFreeLoop` | uncertifiable | no suspension precedes the read, but a loop body is not a completion site: loop bounds are not modelled (ADR 0276) |
| `CrossFilePrefix` | violation | origin in `remote.ts` |
| `MultipleRoles` (apply call) | violation | the same helper's compute and event calls stay clean |
| `StoreProtocolKeys` `cookedDefault` | violation | a cooked ordinary string key |
| `AfterAwaitBody`, `AfterConditionalAwait`, `LoopThenConditionalAwait`, `LoopBackEdge` | uncertifiable | a suspension may precede the read |
| `ForAwait`, `AwaitUsing`, `AwaitOperand`, `DestructureAfterAwait`, `SwitchDefaultAfterTest` | uncertifiable | implicit suspension or unmodelled evaluation order |
| `SuppliedDefaultApply`, `SuppliedStoreDefaultApply`, `DefaultActivationNotProven` | uncertifiable | the default is not shown to run |
| `NestedHelperAfterAwait`, `ConditionalSuspendBeforeChild`, `CrossFileSuffix` | uncertifiable | the child is entered after a possible suspension |
| `AsyncGenerator`, `DormantNestedDefault`, `LazyDefault` | uncertifiable | no body runs, or the default creates a closure |
| `ReassignedTarget`, `TypeOnlyInputs`, `ReplacedSourceDefault` | not a violation | replaced targets, type-only sources and replaced sources prove nothing |
| `StoreProtocolKeys` `thenBody` | uncertifiable (`SC1001`) | a `then` store Get does not warn (ADR 0293), but the literal's computed key leaves the backing `then` unproven (review round 1), so the helper's row is undecided |
| `StoreProtocolKeys` `thenDefault` | uncertifiable (`SC9012`) | the same Get, written in the default |
| `StoreProtocolKeys` `symbolDefault`, `dynamicDefault` | uncertifiable (`SC9012`) | the key may be a symbol or `then`, whose Get does not warn (ADR 0293) |
| `StoreProtocolKeys` `symbolBody`, `dynamicBody` | uncertifiable (`SC1001`) | an undecided key is an undecided row in the helper's summary (ADR 0293, review round 1) |
| `SampledApply`, `SampledDefaultApply`, `TrackedCompute`, `ShadowedName` | not a violation | sampled, tracked or unrelated |

`WrappedExactCall` is meant as a positive but is not proven yet, which is a
recall gap.

The stubs are copied from `prop-head-get`. The sources pass `tsc --noEmit`
against the real installed rc.13 typings.
