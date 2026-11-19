# after-await-review-probes

**Claim (ADR 1097, narrow only).** The two Fable reviews of ADR 1097's widening found arguments and callee bodies that run project code with no call: `in` (a Proxy's `has` trap), `instanceof` (`Symbol.hasInstance`), a computed key (its value's `toString`), and `this` probed with `in`, `for…in` or `instanceof`. Each probe runs in an async memo's continuation, where rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read the project code makes. Every call that runs one keeps its SC1002 row, in the direct, wrapper and IIFE forms.
- `argument_is_inert` takes `proven_primitive` only for a declared name.
- `setter_callee`'s narrow body refuses operators (`primitive_results`), coercions, templates, object properties, computed keys and loops.

Uncertifiable: every call in `Probes`' memo and in `ThisProbes.run`'s memo.
