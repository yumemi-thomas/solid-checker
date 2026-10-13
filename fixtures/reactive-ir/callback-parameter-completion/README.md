# callback-parameter-completion

**Claim (ADR 0277).** A callback passed to a project helper, by reference or as an inline arrow, runs during the call only when the helper calls that parameter on every completion of its own body. A call of the parameter behind a branch, a ternary, a loop or a logical assignment may never happen, so the callback's reads are proof obligations. An accessor passed by reference keeps an uncertifiable row rather than going silent (`execution_role::invokes_parameter_during_call`, `calls_parameter_in_body`, `SemanticLookup::parameter_called_on_every_completion`).

| Case | Finding | Why |
| --- | --- | --- |
| `PassedAlways` | `SC1001` violation | `runAlways` calls `cb()` on every completion |
| `ArrowAlways` | `SC1001` violation | the same with an inline arrow |
| `PassedWhen` | `SC1001` uncertifiable | `runWhen` calls it only when `flag` is set |
| `PassedTernary` | `SC1001` uncertifiable | a ternary arm |
| `ArrowWhen` | `SC1001` uncertifiable | the inline arrow may never run |
| `ArrowEach` | `SC1001` uncertifiable | a loop body, here over an empty list |
| `ArrowWhenUnset` | `SC1001` uncertifiable | `??=` calls it only when the target is nullish |
| `AsyncArrowWhen` | `SC5001` uncertifiable | no pending-read error for a callback that may never run |
| `AfterFailArrow` | `SC1001` uncertifiable | a `never`-returning guard may throw first into the caller's `catch` (ADR 0278) |
| `ReassignedPassed`, `ReassignedArrow` | `SC1001` uncertifiable | the helper replaces its parameter before calling it (ADR 0278) |
| `SpreadBefore` | `SC1001` uncertifiable | an earlier spread may fill the parameter the helper calls (ADR 0278) |
| `CallMember` | `SC1001` uncertifiable | `f.call(thisArg, …)` shifts every argument by one (ADR 0278) |
| `BusMethod`, `ExactInstance` | `SC1001` violation | an unreassigned object-literal method and an exact `new Base()` run the declared method (ADR 0279) |
| `MaybeOverride` | `SC1001` uncertifiable | `makeBase(true)` may return a subclass whose `run` never calls the callback (ADR 0279) |
| `ReplacedRunner` | `SC1001` uncertifiable | the `let` helper binding may have been replaced (ADR 0221, ADR 0278) |

Stubs are copied from `summary-completion-read`; `tsc --noEmit` passes.
