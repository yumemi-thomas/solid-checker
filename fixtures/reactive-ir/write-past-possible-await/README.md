# write-past-possible-await

**Claim.** A write that may follow an await in its own async function (a `catch` or `finally` whose `try` head may throw
before it suspends, code after an await in a branch) runs either on the stack its function was entered on, in the role
the function's entries give it, or past the await on a later task with no owner (Solid's write guard reads the ambient
owner, `@solidjs/signals@2.0.0-rc.17` `dist/dev-shared.js` `setSignal`, and every entry point restores it before a
promise job runs). So when every entry is permissive (a handler) the write is legal on both paths and certified
(`Submit`, `Branch`); when an entry rejects (the component body calls the function) neither path is proven and the
write stays uncertifiable (`BodyCatch`). Control: a write before any await, called from the body, is a proven
`no-write-in-pure-computation` (`BodyBefore`). The fuzzer's `async` contexts (rc.17 fast runtime, 2026-11-03) report no
REACTIVE_WRITE_IN_OWNED_SCOPE past an await in any form. The stub is copied from `fp-exec-await-argument-write`.
