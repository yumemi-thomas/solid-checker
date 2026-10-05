# summary-direct-read

**Claim (ADR 0201).** A strict read attributed through a called function's
summary is a proven untracked read when the read runs while the call does. That
holds when:

- the callee is one synchronous project function;
- the call sits outside any JSX, because a prop getter runs when the consumer
  reads it;
- the read was discovered in the callee itself, as a call written directly in
  its body (not in a nested function or a default parameter), or is the
  accessor argument the callee's own body calls.

Every other attributed read stays uncertifiable (ADR 0185's precision fix,
`e101c6989`).

| Case | Finding | Why |
| --- | --- | --- |
| `CallsReadNow` | `SC1001` violation | `readNow` reads `count()` directly in its body |
| `CallsReadArgument` | `SC1001` violation | `readArgument` calls its accessor argument in its body |
| `CallsReadLater` | none | the read is in a returned closure, called in JSX |
| `CallsReadAfterAwait` | `SC1001` uncertifiable | an async helper |
| `CallsReadDefault` | none | a default-parameter read is not attributed (a missed finding, not a claim) |
| `CallsReadThroughHelper` | `SC1001` uncertifiable | the read is two calls deep |
| `CallsInJsxProp` | none | the prop getter runs where `Show`'s JSX reads it |

Runtime: `fixtures/app-patterns-misuse` cases `body-read-through-local-helper`,
`body-read-through-hook`, `hook-reads-own-signal`,
`hook-reads-accessor-argument` and `effect-apply-read-through-helper` raise
`STRICT_READ_UNTRACKED` in Chrome on rc.13, and their correct twins raise
nothing.

The stubs are copied from `forwarded-event-prop`.
