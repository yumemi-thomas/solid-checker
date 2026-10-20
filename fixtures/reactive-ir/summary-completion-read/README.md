# summary-completion-read

**Claim (ADR 0276).** A strict read attributed through a called function's summary is a proven untracked read only when it runs on every normal completion of the call (`interproc::site_on_every_completion`); behind a branch, early return, thrown/`never`/`asserts` guard (ADR 0278/0279), loop, logical assignment or a default's own branch it is uncertifiable. Scaffolding from `summary-direct-read`; `createStore` from `async-call-prefix`.
