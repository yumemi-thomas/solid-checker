# rc9-reexport-gap-aliased

An aliased import, `sharedConfig as config`: the local name is not the export,
and the census reads the export name the import declaration spells
(`rc9-reexport-gap-none` explains the gap and the stubs, which are the same
here).

Expected: the `SC9014` notice carries the re-export gap, naming
`sharedConfig` ("this project uses sharedConfig from solid-js"). Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).
