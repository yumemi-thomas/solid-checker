# release-triple-jsx-only-rc3

A project whose only contact with Solid is JSX, compiled with
`jsxImportSource: "@solidjs/web"` on an unaudited `2.0.0-rc.3` triple. No
module reference in `App.tsx` names a Solid package; the JSX runtime import is
implicit.

The checker does not read `jsxImportSource`: the dialect's compiler lowers
every JSX expression in every analyzed file onto its own runtime. So any JSX is
an answer the vocabulary gives about that runtime, and the release notice is
due whatever the configured import source is
(`rust/crates/solid-facts-backend/src/release_scope.rs`, `solid_use`).

Expected: the `SC9014` notice and nothing else. Asserted by
`the_release_notice_is_due_only_for_a_project_that_uses_solid`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).

## Stubs

`node_modules/@solidjs/web/jsx-runtime.d.ts` and
`node_modules/solid-js/types.d.ts` are the `write-scope-roots-rc3` stubs: the
`JSX.Element` and children-attribute lines of `@solidjs/web@2.0.0-rc.3`
`types/jsx.d.ts`, byte for byte, and `solid-js@2.0.0-rc.3` `types/types.d.ts`
whole. `node_modules/solid-js/index.d.ts` keeps line 7 of `types/index.d.ts`,
the `Element` re-export. No rule reads JSX typing and the case renders no
intrinsic element. `tsc --noEmit` (5.9.3, `strict`, bundler resolution) is
clean against these stubs, and against the published rc.3 install with
`skipLibCheck` (the audited archive does not carry `csstype`, which the
published `types/jsx.d.ts` imports).
