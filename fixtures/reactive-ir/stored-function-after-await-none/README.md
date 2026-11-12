# stored-function-after-await-none

**Claim (ADR 0964).** The obligation at an unresolved invocation asks the region of SC1002: a tracked async computation with something after a suspension. This program holds none (a synchronous memo iterates a stored list and calls an object's `emit`; another calls a slot the program assigned, `.call` on a stored element, a stored tag and a stored constructor: batch 10c's forms), so no SC1002 row stands. SC1001's rows are as before.
