# unconditional-structural

ADR 1042: a call under a label, in a finalizer or a `try` block no `catch` guards, in a `do` body, or in a loop whose first
test is true runs at least once on every run of its function, so a `wrapUse` holding `onCleanup` that a handler reaches
unowned is NO_OWNER_CLEANUP (SC4001). The brief's counterexamples stay uncertifiable (an early `break`/`continue`/`return`,
the handler arm, a loop whose test no fact decides, a `for … of` body, a block a `catch` guards). `LabelRead` stays uncertifiable
(`summary-read-unplaced`): a read reached through a function summary is placed by the Type Facts producer's exactly-once
whitelist, which this ADR leaves unchanged (the same read with no label is a violation).

- `Label`: violation
- `Finalizer`: violation
- `TryNoCatch`: violation
- `DoBody`: violation
- `WhileTrue`: violation
- `ForNoTest`: violation
- `LabelledLoop`: violation
- `CatchAndFinalizer`: violation
- `LabelInFinalizer`: violation
- `EarlyBreakOuter`: uncertifiable
- `HandlerArm`: uncertifiable
- `FinalizerEarlyReturn`: uncertifiable
- `DoBreakFirst`: uncertifiable
- `WhileCondition`: uncertifiable
- `WhileContinueFirst`: uncertifiable
- `ForOfBody`: uncertifiable
- `CatchGuardedBlock`: uncertifiable
- `LabelRead`: uncertifiable (SC1001, the producer's whitelist)
