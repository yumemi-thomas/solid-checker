# Defined caller input after a local undefined default

The next Corvu graph refusal is the read of parameter 1's `concat` member in
`@floating-ui/utils@0.2.12 ./dom:getOverflowAncestors`. Its JavaScript performs
`if (list === void 0) { list = []; }` before both calls. Neither an unwritten
binding nor an unconditional original-input witness describes those reads.

Protocol 45 adds `undefinedDefault` to an exact initial parameter-read row.
The producer binds the strict-undefined guard and assignment locations, the
signature's exact parameter declaration, and the first subsequent reference.
It admits only one assignment to that binding in the entire body, under a
direct body-level guard without an else, assigning a local empty array.
Parameter defaults, rest/destructuring, duplicate parameter symbols, nested
callables, `arguments`, `eval`, `with`, redeclarations and additional stores
remain excluded. Guard and assignment must resolve to the same parameter
symbol. A spelling of `undefined`, loose equality, or an effectful void operand
does not satisfy the guard.

Only the parameter's first reference other than that exact guard operand and
assignment target can carry the fact. It must follow the guard and be an
uncaptured property use. This additional boundary is necessary: a function
can save `const missing = input === void 0` before defaulting, then read
`input.concat` only when `missing`. Conditional identity alone would incorrectly
count that local-only read. An overlay regression demonstrated this defect in
the initial candidate; the retained producer and native regressions reject it.
Aliases and predicates before the candidate read also prevent the new fact.
The first Corvu diagnostic used the broader candidate and is not coverage
evidence for this corrected implementation.

For every defined caller argument the guard is false, and the exhaustive
store census proves the original value survives. For an undefined argument
the read instead has local origin; it is not evidence of a caller-input read.
The partial positive read domain can describe the former without claiming the
latter is absent. The native verifier therefore requires an explicit zero
minimum, the exact member path and callee occurrence, and a non-composed read.
It cannot use this fact for its whole-root shortcut, guaranteed execution,
domain closure, a different member, or an unwritten binding. The exact
origin-and-callee witness discharges an unasserted operation-input root directly:
an optional declared argument need not have that member unconditionally for
this partial read to exist. A shape or callability assertion still follows its
independent verification path, as do the other operation obligations. This
also applies to existing exact initial-member witnesses; it does not apply to
the existence-only whole-parameter shortcut.

The client validates the same-path guard/store/use envelope and rejects any
combination with the positional or first-iteration markers. Producer and
consumer move together with the schema digest and handshake. The schema also
corrects the obsolete `conditional` spelling to the actual `positional` field;
a transport-key regression covers that drift. No contract or receipt format,
trust policy, coverage denominator, or package-resolution identity changes.

The corrected producer cases pass, and the two armed native tests pass in
20.92 seconds. They include the real optional-array declaration shape,
guaranteed-read and whole-root refusals, later writes, a same-binding var
initializer, and the fallback-only counterexample. Client envelope controls
reject mismatched paths, store/guard ordering and incompatible markers.
Full `make verify` passes with actual exit 0, TOTAL 166.60 seconds and no
`FAILED during step` marker. Existing snapshots are unchanged.

The [corrected Corvu measurement](../package-contract-v2/phase21/2026-09-08-defined-input-recovery.md)
publishes both root cases with authenticated ordinary-consumer selection:
one refused-to-complete row and two new artifact cases. The full corpus rerun
is active; preservation and any additional gains are not yet measured.
