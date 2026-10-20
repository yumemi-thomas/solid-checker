# release-triple-jsx-only-rc3

A project whose only Solid contact is JSX (`jsxImportSource: "@solidjs/web"`) on an unaudited `2.0.0-rc.3` triple: the dialect's compiler lowers JSX onto its own runtime, so the release notice is due (`release_scope.rs`). Expected (asserted by `the_release_notice_is_due_only_for_a_project_that_uses_solid` in `dialects_process.rs`): `SC9014` and nothing else. Stubs are the `write-scope-roots-rc3` ones (the `JSX.Element`/children lines of `@solidjs/web@2.0.0-rc.3` `types/jsx.d.ts` byte for byte; `solid-js@2.0.0-rc.3` `types/types.d.ts` whole).
