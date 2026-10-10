# rc9-reexport-gap-named

A named import of `createErrorBoundary`, one of the five names
`solid-js@2.0.0-rc.9`'s typings re-export without declaring
(`rc9-reexport-gap-none` explains the gap and the stubs, which are the same
here). Under `skipLibCheck` the import is untyped and the call in `source.ts`
is not the primitive, so the gap is this project's.

Expected: the `SC9014` notice carries the re-export gap, its sentence ends
"this project uses createErrorBoundary from solid-js", and an evidence step
locates the import. Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`); the snapshot
records spans only.
