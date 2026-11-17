# returned-closure-unrun

**Claim (ADR 1020).** A function literal that a callback of a dialect primitive returns (an arrow's expression body or a `return`'s argument) is a value the runtime hands on: an effect's apply receives it, a memo stores it. It runs only where something calls that value, and reach analysis does not read that, so a site in it is undecided: a violation there is `site-reach-undecided`, an obligation stays, nothing becomes clean.

Violation: `DirectSync` (SC7002: a `sync: true` memo whose compute returns a Promise; Chrome reports SYNC_NODE_RECEIVED_ASYNC).

Obligations (uncertifiable): `ReturnedToApply` and `ReturnedUnderUntrack` (the effect's compute returns a closure the apply only reads; Chrome is silent: the false positive of main, judged `unexecuted`), `ReturnedFromMemo` (a memo's value nothing calls; silent).

Known miss: `ReturnedAndCalled` (the apply calls the closure, so the memo is created inside the apply: Chrome reports PRIMITIVE_IN_EFFECT_CALLBACK there, which SC6002 covers as uncertifiable): SC7002 is uncertifiable, never a wrong answer. A cleanup an apply returns is not withheld: the runtime calls it (`Dialect::callback_return_is_value` answers only for a compute).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/returned-closure-unrun`.
