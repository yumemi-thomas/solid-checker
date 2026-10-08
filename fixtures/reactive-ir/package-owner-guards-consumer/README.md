# Exact argument guards for owner and leaf requirements

This hand-stated contract isolates Literal(true), Property(resize, callable)
and Literal(length = 2). For each export, module and leaf calls have four
cases: proving literal (violation), disproving literal (clean), nonliteral
argument (uncertifiable), and spread argument (uncertifiable).

The package manifest is byte-identical to package-own-tracked-read-consumer.
authorize-contract.json keeps that fixture's identity/closure bytes and
changes only export identities; no obsolete accepted catalog is included.
The package is synthetic: its declaration describes its hand-stated branch,
not a reduced real-package typing. Solid signatures are copied from the
existing package-leaf-registration-consumer stub (published rc.9 declarations).

Expected guard findings are recorded in EXPECTED.json; this is a semantic
expectation, not an observed findings snapshot. The copied rc.0 dialect stub
can also produce unaudited-solid-release (SC9014), as in the source fixture.
Do not silently repin it or claim the entire fixture has zero findings.

Stage this directory as fixtures/reactive-ir/package-owner-guards-consumer
with its .gitignore exceptions (both supplied in PATCH.diff). The lead must
authorize via the coverage harness, inspect the non-updating result and only
then generate the focused snapshot. Analyze without authorization once to
confirm that rejection yields SC9005 instead of these downstream guard results.
