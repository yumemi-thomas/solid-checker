# stored-throw-class-member-wrapper

**Claim (ADR 0914).** A static `dynamic` source that returns a Promise throws a plain error on every run (rc.21), and ADR 0893 needs a run whose throw reaches a report. The walk finds no entry of a class constructor or method (`new` is no plain call; nothing names them), so it stops there. When every place it stopped is written inside a memo nothing reads, that memo is the run in view, as it is for a site written there: uncertifiable, `computation-error-unobserved`.

Uncertifiable: `FuzzShape` (the fuzzer's lg-recall program `lg-dynamic-static-promise-flush-read-stored-x-wrapper-handoff-assign-this`, Chrome: legal; a violation before ADR 0914), `MethodInMemo`, `DirectInMemo` (the site in the stored arrow: ADR 0893's lexical fallback, unchanged).

Violations: `ReadMemo` (an effect reads the memo), `InBody` (the class in the component body).

Premises: `@solidjs/web` and `solid-js` 2.0.0-rc.21 (stub `node_modules`).
