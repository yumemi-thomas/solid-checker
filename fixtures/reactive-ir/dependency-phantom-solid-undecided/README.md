# dependency-phantom-solid-undecided

**Claim (ADR 0981).** A package whose manifest names no Solid package and whose closure cannot be read is undecided, not clean: the scan does not select it (`export * from "./missing.js"` resolves to no source file), and the values it returns are the unselected rule's: the read `location.search` is an uncertifiable SC1001 (`package-value-read`), never silent and never a violation.

The same program and package as `dependency-phantom-solid`, with one more line in the entry that resolves to nothing. The refusal is recorded as undecided (`closure-unavailable`), apart from the plain `no-demand` answer of a package whose closure was read and imports no Solid package (`dependency-phantom-solid-plain`).

**ADR 1114 family 3 slice E.** Where a loaded package writes a built-in's prototype and may reach a Solid action, every operation that may run a written member is uncertifiable (`package-may-call-action:<reach>:prototype-<kind>`): a member call whatever its declaration, a call, constructor or tag the library declares or nothing resolves, a converting operator, a template substitution, an array pattern, `await`, `using` and a JSX value. A twin above called silent that makes such an operation in an owned scope under such a writer is uncertifiable now; a twin in a handler stays silent.
