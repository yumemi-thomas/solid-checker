# param-default-leaf

**Claim (ADR 1180).** A direct call with no spread argument that passes fewer arguments than a parameter's index runs that
parameter's default on its own stack. When the default is a whole call (a flush, a creation, `onCleanup`, or a helper that
performs one), the leaf walk walks it as a call in the body. That makes SC3001 a violation at the call in the tracked
effect's callback (lines 17, 24, 31, 112, 119), through the value, helper and namespace forms.

Stays uncertifiable:
- a passed argument (`valued(1)`, 40): a passed value may be `undefined` at run time;
- a spread (50) and an `undefined` argument (101);
- a default after an earlier default that calls (63): a throwing call would stop the list first;
- a method's default reached through an object literal (85, 92): the walk does not resolve the method.

A shadowed local `flush` is clean.

Chrome: case `runtime-oracle/param-default-leaf`. The creation and the cleanup are covered-violation. Each flush throws at its
default, and a thrown error reaches the report with no frames, so the oracle ledgers the call-site violations as `attribution`.
