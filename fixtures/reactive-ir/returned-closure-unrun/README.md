# returned-closure-unrun

**Claim (ADR 1020).** A function literal that a callback of a dialect primitive returns (an arrow's expression body or a `return`'s argument) is a value the runtime hands on: an effect's apply receives it, a memo stores it. It runs only where something calls that value, and reach analysis does not read that, so a site in it is undecided: a violation there is `site-reach-undecided`, an obligation stays, nothing becomes clean.

Violations: `DirectSync` (SC7002: a `sync: true` memo whose compute returns a Promise; Chrome reports SYNC_NODE_RECEIVED_ASYNC).

Obligations (uncertifiable): `ReturnedToApply` and `ReturnedUnderUntrack` (the effect's compute returns a closure the apply only reads; Chrome is silent: the false positive of main, judged `unexecuted`), `ReturnedFromMemo` (a memo's value nothing calls; silent).

Violations too, since the narrowing after #271's smoke census: `AppliedVoidCall` and `AppliedVoidCallUntrack` (the apply calls the value, `void v()`, so the closure runs: MISSING_EFFECT_FN at the effect created there with no effect function, SC7001; Chrome reports it). The value is withheld only where it is provably never called: the apply's first parameter is absent or only the operand of `void` (`void v`), or a memo's result is discarded or only read through `void`/`typeof` of a call with no argument. A cleanup an apply returns is not withheld either: the runtime calls it (`Dialect::callback_return_is_value` answers only for a compute).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/returned-closure-unrun`.
