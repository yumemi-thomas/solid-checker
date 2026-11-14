# after-await-then-handler

**Claim (ADR 0976; S3 of the review of #233).** A promise handler is code written after a suspension of the function that holds the chain, `async` or not: `function work() { return pause().then(() => source()); }` is the compute of `createMemo` once a project wrapper hands it on, and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read in the handler. main was silent: the file has no `async` function, and the walks follow a chain only from a recognized computation.

A handler is the function literal (or the function named by reference) given to `then` (either slot), `catch` or `finally`, where the callee is `Promise.prototype.*` / `PromiseLike` by its resolved declaration, or a member named by a literal key on a receiver the program does not prove non-thenable. A reading handler, whatever starts the chain, is a site: it ends in a row, an SC9012, the compiler's discard or a typed reason, else an uncertifiable SC1002 row.

| function | outcome |
|---|---|
| `viaThen`, `viaCatch`, `viaFinally`, `viaReference` (`then(reader)`) | SC1002 uncertifiable at the read in the handler |
| `viaComputed` (`pause()["then"](…)`) | SC9012 uncertifiable at the call (`handed-member-dispatch`): the pass does not resolve a computed key |
| `viaPure` (a handler that reads nothing and calls no project function) | no row |
| `standalone` (no computation starts it) | SC1002 uncertifiable: there is no census reason, and a function no computation starts is still a site |

The process test `a_promise_handler_is_code_written_after_a_suspension_of_the_function_that_holds_the_chain` pins the exact rows. The oracle case `fixtures/runtime-oracle/after-await-then-handler` is the Chrome twin.
