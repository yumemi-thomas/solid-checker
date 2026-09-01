# Importing JSON is data, not an analysis failure

`index.mjs` imports its own `package.json` for a name and a version string.
This mirrors the real shape in `@solidjs/start@2.0.3`
(`dist/shared/dev-toolbar/index.jsx`), which was the first artifact to hit it.
The sibling `index.d.mts` keeps the declaration surface format-faithful to the
selected `.mjs` runtime target; declaration resolution must not borrow a
cross-format `.d.ts` file or treat the runtime source as its declaration.

The target is legitimate ESM data rather than JavaScript, so contract
generation must not refuse the entrypoint over it, and the fields read from it
are plain data: `packageName` is a callable with no operations and
`packageVersion` is `plain`. No reactive read, no callback, no owner
requirement is manufactured from a JSON module.

`asset-import` pins the neighbouring case where the imported target is not a
module at all (a stylesheet); the two answers are deliberately different --
the asset is opaque, the JSON is inert.
