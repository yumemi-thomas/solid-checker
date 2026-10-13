# rc9-reexport-gap-none

The rc.9 re-export gap is scoped to the names it loses, and this project
reaches none of them.

`solid-js@2.0.0-rc.9`'s `types/index.d.ts` re-exports `$DEVCOMP` from
`./client/core.js` (`:3`) and `sharedConfig`, `createErrorBoundary`,
`createLoadingBoundary` and `createRevealOrder` from `./client/hydration.js`
(`:8`), and neither file declares them (the rc.9 review § 5, N2). Under
`skipLibCheck` an import of one is untyped and a call through it is not the
primitive. `KNOWN_GAPS` scopes that gap to those five exports
(`rust/crates/solid-dialect/src/solid_2/releases.rs`), and
`rust/crates/solid-facts-backend/src/release_scope.rs` decides from the import
facts whether a project reaches one.

`source.ts` imports `untrack` by name and reads `S.untrack` through a namespace
import: every use names an export, and none is one of the five. Expected: the
`SC9014` notice does **not** carry the re-export gap. It still fires, for the
rc.9 triple's release-wide gaps (signals' negative rows), which is why
`rc9_re_export_gap_is_due_only_where_a_project_reaches_it`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`) asserts the gap
list rather than the notice's presence; the snapshot records spans only.

The five `rc9-reexport-gap-*` positives are `-named`, `-aliased`,
`-namespace-member`, `-namespace-escape` (fail closed) and `-project-reexport`.

## Stubs

`node_modules/` holds the rc.9 triple, reduced. `solid-js/types/index.d.ts`
keeps rc.9's two defective re-export statements, narrowed to the five names
and `untrack`, from files that do not declare them, so the defect is the
published one: `tsc --noEmit` (5.9.3, `strict`, bundler resolution) reports
the same five TS2305 inside the package with `skipLibCheck: false` as the
published rc.9 does, and is clean with `skipLibCheck: true` on every
`rc9-reexport-gap-*` source, against these stubs and against the published
`solid-js`, `@solidjs/signals` and `@solidjs/web` `2.0.0-rc.9`. `untrack`'s
declaration is byte-faithful to `@solidjs/signals@2.0.0-rc.9`
`dist/types/core/core.d.ts:105`, moved to the stub's `dist/types/index.d.ts`.
`@solidjs/web` is a manifest only: nothing here imports it.
