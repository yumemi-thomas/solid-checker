# private-component-layers

**Claim (ADR 0506).** Reachability follows a function literal that a call invokes (an IIFE's callee) or receives as an argument (a project helper's callback), so a write in one inside a component that only a parent renders by JSX (not exported, never a root) is judged like the same write in an exported component. Before, only functions nested in an exported declaration were roots, so these writes were dropped and the program certified clean (fuzzer mount axis: 364 false cleans per mount). Violations: `ByIife`, `ByHelper`, `ByClosure`. Uncertifiable: `ByNeverHelper` (the helper never calls its callback; reaching a function proves nothing about when it runs).

Stubs: `node_modules` and `jsx.d.ts` copied from `effect-apply-first-run` (rc.17 published declarations).
