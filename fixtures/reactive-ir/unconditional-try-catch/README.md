# unconditional-try-catch

ADR 1048: the first statement of a `try` block a `catch` guards runs at least once when nothing before the call can throw
(a hoisted function declaration nothing writes, or a package import, called with literals, functions or arrows), and a
`break` that leaves only a loop or `switch` written before the call is no way out of its function. So `wrapUse`/`onCleanup`
reached unowned from a handler is NO_OWNER_CLEANUP (SC4001). The counterexamples stay uncertifiable.

- `GuardedFirst`: violation
- `GuardedWithFinalizer`: violation
- `GuardedImport`: violation
- `AfterInnerBreak`: violation
- `AfterInnerSwitch`: violation
- `GuardedAfterCall`: uncertifiable
- `GuardedConstCallee`: uncertifiable
- `GuardedArgumentThrows`: uncertifiable
- `HandlerArm`: uncertifiable
- `LabelledBreakOut`: uncertifiable
