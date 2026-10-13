# rc9-reexport-gap-namespace-escape

The fail-closed case. The namespace object escapes into an array, so which of
its exports is reached later is not something the facts can bound, although
the one member read in `source.ts` (`S.untrack`) names a declared export
(`rc9-reexport-gap-none` explains the gap and the stubs, which are the same
here). Every reference to the namespace binding that is not a member read
naming one property escapes the same way: an argument, a destructuring, an
`export { S }`, a type (`typeof S`), or a computed key that is not a literal.

Expected: the `SC9014` notice keeps the re-export gap, which says the project
uses solid-js in a way that does not name the exports it reaches, and an
evidence step locates the escaping reference. Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).
