# flush-in-call-entries

**Claim (ADR 1199).** A `flush()` with no census class of its own, written where it runs whenever its function runs, in a function every run of which is a plain call the program writes (or a component prop's literal, run at the child's calls of the prop, ADR 1171; or an in-place layer's one call), is asked at each of those calls (`flush_accounting.rs` `entries_placed`): when every call has a position class that rules every code out (a timer's callback, an await's continuation, module scope, a test runner's callback), or, within a depth of 4, its own function's calls do, the site is ended by `FlushSafe::EntriesPlaced`. The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason ends any site here.

Clean: `Twice`, `TimerAndAwait`, `Chain`, `PropLater`, `AfterAwaitIife` (an in-place layer's one call, `LayerRun::InPlace`, after an await).

Uncertifiable (fail closed): `BeforeAwaitIife` (the IIFE's call runs before the await, on the component body's stack), `InApply` (one call is in an effect's apply: SC2008's own row decides there), `InHandler` (an event handler's delivery has no reason, owner ruling 5a), `ViaMember` (a reference that is no call: the entries are not all known), `PropNow` (the child also calls the prop in its body, which has no class).
