# rc9-reexport-gap-project-reexport

A project module re-exports two of the five names, one aliased
(`export { createRevealOrder, $DEVCOMP as devComponent } from "solid-js"` in
`boundary.ts`), and `source.ts` imports from that module rather than from
`solid-js` (`rc9-reexport-gap-none` explains the gap and the stubs, which are
the same here). The re-export is what reaches the gap: it names the exports
it takes from `solid-js`, so they are the ones counted, whether or not an
importer uses them.

Expected: the `SC9014` notice carries the re-export gap, naming both
("this project uses createRevealOrder and $DEVCOMP from solid-js"), with one
evidence step per name located in `boundary.ts`. Asserted by
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).
