# flush-in-place-layers

**Claim (ADR 1090).** A `flush()` written in exact in-place layers (`climb_in_place_layers`: a local closure called once, a named helper called once, an IIFE) inside a reviewed scheduler's callback runs on the callback's stack, as a flush written directly there does: the flush accounting asks its class questions at the outermost layer's call. The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason ends any site here.

Clean: `Closure`, `Helper`, `Iife` (Chrome rc.21: the legal fuzz cells `lg-flush-*-timer-*-{closure,helper,iife,...}` are clean).

Uncertifiable (fail closed): `HandedOn` (the closure is also pushed into an array: a run of it may be elsewhere), `Shared` (the helper is also called from an action's step, where `flush()` throws FLUSH_IN_ACTION; the producers' own row there decides that call).
