# argument-sensitive-leaf

**Claim (ADR 0280).** The leaf-scope walk decides a helper's parameter tests from the arguments of the helper call in the leaf callback, so the `@solid-primitives/timer` shape is a proven violation for exactly the operation the delay selects (a number: `onCleanup`; an accessor: `createEffect`) and `number | (() => number)` gives two uncertifiable findings.
