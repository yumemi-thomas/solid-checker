# release-triple-solid-free-rc3

A project with no Solid code, analyzed beside an unaudited `2.0.0-rc.3`
triple. It is the shape of kobalte's `packages/tailwindcss` (the phase 22
consumer-effect measurement,
`docs/package-contract-v2/phase22/2026-09-27-consumer-effect-of-rc9-audited.md`,
spot check 4): the dialect walk finds the triple above the project, and
nothing in the project reaches it.

`index.ts` imports a project module (`./label`) and a package,
`plain-format`, whose installed dependency closure is `plain-leaf` and an
optional dependency that is not installed. No module reference names
`solid-js`, `@solidjs/signals` or `@solidjs/web`, there is no JSX, and no
installed package the project reaches depends on one of them, so the analysis
asks the vocabulary no release-dependent question
(`rust/crates/solid-facts-backend/src/release_scope.rs`, `solid_use`).

Expected: **no finding** and `certified`; in particular no `SC9014`. The
controls, on the same triple, each keep the notice:
`release-triple-solid-import-rc3` (a `solid-js` import),
`release-triple-jsx-only-rc3` (JSX alone) and
`release-triple-solid-dependent-rc3` (a dependency that peer-depends on
`solid-js`). Asserted by `the_release_notice_is_due_only_for_a_project_that_uses_solid`
(`rust/crates/solid-facts-backend/tests/dialects_process.rs`).

`node_modules/` holds the three Solid manifests at `2.0.0-rc.3` and the two
plain packages; `tsc --noEmit` (5.9.3, `strict`, bundler resolution) is clean.
