# unconditional-literal

ADR 1046: a counter `for` whose literal bounds pass the first test, and the case a literal `switch` enters (the first
strictly equal one, its call before any `break`), run at least once on every run of their function, so a `wrapUse` holding
`onCleanup` that a handler reaches unowned is NO_OWNER_CLEANUP (SC4001). The brief's counterexamples stay uncertifiable.

- `CounterLoop`: violation
- `CounterThree`: violation
- `CountDown`: violation
- `LiteralSwitch`: violation
- `StringSwitch`: violation
- `ZeroIterations`: uncertifiable
- `BreakFirst`: uncertifiable
- `ContinueFirst`: uncertifiable
- `BoundNotLiteral`: uncertifiable
- `SwitchNotLiteral`: uncertifiable
- `SwitchMiss`: uncertifiable
- `FallThrough`: uncertifiable
- `DefaultArm`: uncertifiable
- `CaseBreakFirst`: uncertifiable
