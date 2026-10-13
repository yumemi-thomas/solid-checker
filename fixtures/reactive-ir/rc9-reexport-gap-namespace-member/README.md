# rc9-reexport-gap-namespace-member

A namespace import whose every use is a member read. `S.untrack` names an
export rc.9 declares and reaches nothing; `S.createLoadingBoundary` names one
of the five it does not (`rc9-reexport-gap-none` explains the gap and the
stubs, which are the same here). A namespace reference is matched to its
import by the binder's own resolution, so a shadowing `S` is not the import.

Expected: the `SC9014` notice carries the re-export gap, naming only
`createLoadingBoundary`. Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).
