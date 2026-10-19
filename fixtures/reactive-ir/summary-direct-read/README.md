# summary-direct-read

**Claim (ADR 0201).** A strict read attributed through a called function's summary is a proven untracked read when it runs while the call does: one synchronous project callee, a call outside JSX, and a read discovered directly in the callee's body (not a nested function or default) or the accessor argument its body calls; later closures and JSX prop getters are none; async helpers are uncertifiable. Runtime: the `fixtures/app-patterns-misuse` hook/helper cases raise `STRICT_READ_UNTRACKED` on rc.13.
