# Entrypoint discovery across every shape one `exports` map can take

One manifest publishing four public entrypoints, each reaching the generator by
a different route:

- `.` is a nested conditional map, so it produces three artifact cases --
  `node/import` on `node.ts`, `browser/import` and `default` on `index.ts`.
  The two `index.ts` cases describe the same three exports (`rootValue`,
  `rootConstant`, `default`), which is what makes the `node` case's single
  different export attributable to the condition rather than to the file.
- `./state` reaches `state.ts`, whose surface is a `export *` re-export of a
  sibling joined with a local constant, so both `stateValue` and
  `stateConstant` have to appear on one case.
- `./features/*` is a wildcard subpath; `./features/alpha` is the target the
  walk has to discover, since no literal key names it.
- `./empty` is published but its target exports nothing, and it is *refused*
  rather than described -- an entrypoint with no runtime ESM export is not the
  same document as an entrypoint whose exports are all uninteresting. The
  refusal is recorded in the artifact-case census while the other four
  entrypoints still certify, so one dead subpath does not refuse the package.

`version` is deliberately `1.2.3` rather than `1.0.0`: the package identity in
the contract is compared against the manifest, not against a default.
