# argument-sensitive-reads

**Claim (ADR 0280).** A read a helper performs on some of its completions is decided for one call when that call's arguments decide the branch: proven where it always runs, dropped where it cannot run, uncertifiable otherwise. The same holds for a callback the helper calls on some paths. Only the helper's own reads are decided, over an exact argument mapping, for parameters the helper never writes (`argument_values::parameter_values`, `solid_facts::ast::argument_site_standings`).

| Case | Finding | Why |
| --- | --- | --- |
| `WhenFunction` | `SC1001` violation | a function skips `typeof value === "number"` and reaches `count()` |
| `GuardedTrue` | `SC1001` violation | `flag ? count() : 0` with `true` |
| `FallbackOmitted` | `SC1001` violation | an omitted argument is `undefined`, so `??` reads its right side |
| `AfterGuardReady` | `SC1001` violation | `ready` is `true`, so the `never` guard's arm is dead |
| `PassedWhenTrue` | `SC1001` violation | `runWhen(true, local)` calls `cb()` |
| `WhenNumber`, `GuardedFalse`, `FallbackGiven`, `PassedWhenFalse` | none | the arguments rule the read out |
| `WhenEither`, `GuardedUnknown`, `PassedWhenUnknown` | `SC1001` uncertifiable | the argument decides nothing |
| `WrittenTrue` | `SC1001` uncertifiable | the helper writes its parameter, so the argument decides nothing |
| `AfterGuardNotReady` | `SC1001` uncertifiable | the `never` guard runs, and the caller may catch it |

Stubs are copied from `summary-completion-read`; `tsc --noEmit` passes.
