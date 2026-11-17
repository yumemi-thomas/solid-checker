# helper-throwing-default

**Claim (ADR 1010).** A helper that calls its callback parameter in its body runs it only when the body runs. A parameter default runs before the body whenever its argument is missing or `undefined`, and a default that may run code (`_gate = stopGate()`) may throw there: the body, and the callback's call in it, is not proven to run. The helper's call of its parameter is a proven run only when every default is inert (a function or primitive literal, `{}`, `[]`, `undefined`) or the call supplies, at that position, a function or primitive literal.

Violations: `InertDefault` (a literal default), `DefaultSupplied` (the throwing default's argument is a literal): SC2001 at the write (Chrome reports REACTIVE_WRITE_IN_OWNED_SCOPE).

Obligations (uncertifiable): `ThrowingDefault` and `ThrowingDefaultDeclared` (the default throws before the body; Chrome is silent: the false positive of main, SC2001 at the write), `DefaultMaybeSupplied` (the argument is a name that may be `undefined`; Chrome is silent too, the value is `undefined`).

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/helper-throwing-default`.
