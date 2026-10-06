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
| `SuspensionFreeLoop`, `BeforeImplicitSuspension`, `PreferPrefixOrigin` | violation | no suspension precedes the read |
| `CrossFilePrefix` | violation | origin in `remote.ts` |
| `MultipleRoles` (apply call) | violation | the same helper's compute and event calls stay clean |
| `StoreProtocolKeys` `cookedDefault` | violation | a cooked ordinary string key |
| `AfterAwaitBody`, `AfterConditionalAwait`, `LoopThenConditionalAwait`, `LoopBackEdge` | uncertifiable | a suspension may precede the read |
| `ForAwait`, `AwaitUsing`, `AwaitOperand`, `DestructureAfterAwait`, `SwitchDefaultAfterTest` | uncertifiable | implicit suspension or unmodelled evaluation order |
| `SuppliedDefaultApply`, `SuppliedStoreDefaultApply`, `DefaultActivationNotProven` | uncertifiable | the default is not shown to run |
| `NestedHelperAfterAwait`, `ConditionalSuspendBeforeChild`, `CrossFileSuffix` | uncertifiable | the child is entered after a possible suspension |
| `AsyncGenerator`, `DormantNestedDefault`, `LazyDefault` | uncertifiable | no body runs, or the default creates a closure |
| `ReassignedTarget`, `TypeOnlyInputs`, `ReplacedSourceDefault` | not a violation | replaced targets, type-only sources and replaced sources prove nothing |
| `StoreProtocolKeys` `then`/symbol/dynamic | uncertifiable | protocol and dynamic store keys |
| `SampledApply`, `SampledDefaultApply`, `TrackedCompute`, `ShadowedName` | not a violation | sampled, tracked or unrelated |

`WrappedExactCall` is meant as a positive but is not proven yet, which is a
recall gap.

The stubs are copied from `prop-head-get`. The sources pass `tsc --noEmit`
against the real installed rc.13 typings.
