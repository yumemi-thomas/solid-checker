# after-await-promise-unconsumed-eval

**Claim (ADR 1090, G1 residual, review F3).** `AfterAwaitSafe::PromiseUnconsumed` holds no site in a program with a direct `eval` or a code-evaluating call: the string may call the function where its promise is consumed. Here `eval("fire()")` runs in a memo's compute, so Chrome reports UNTRACKED_READ_AFTER_AWAIT at the read in `fire`. Uncertifiable: the read in `fire`. Twin of after-await-promise-unconsumed, where `fire` is clean.
