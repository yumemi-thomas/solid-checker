# release-triple-solid-import-rc3

The control for `release-triple-solid-free-rc3`: the same project shape on the
same unaudited `2.0.0-rc.3` triple, with one named import from `solid-js`.
The project uses the runtime, so every open gap of the installation is its
own.

Expected: the `SC9014` notice, with the rc.3 triple's one gap (older than the
audited release), and nothing else. Asserted by
`the_release_notice_is_due_only_for_a_project_that_uses_solid`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).

`solid-js.d.ts` declares `untrack` byte-faithful to
`@solidjs/signals@2.0.0-rc.3` `dist/types/core/core.d.ts:75`, which
`solid-js`'s `types/index.d.ts:1` re-exports; `tsc --noEmit` (5.9.3, `strict`,
bundler resolution) is clean against it and against the published rc.3
install. `node_modules/` holds the three manifests at `2.0.0-rc.3`.
