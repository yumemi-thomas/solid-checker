# A wildcard export key that enumerates assets alongside real modules

This is the published shape of `@kobalte/solidbase`, whose `"./default-theme/*"`
key expands over a directory holding sourcemaps, JSON, and CSS next to the
actual JavaScript. The finite wildcard census walks the artifact's files, so
every one of them becomes an artifact-case candidate.

`./assets/widget.js` certifies: it is an executable module, and it has no
declaration sibling, so the runtime file is its own declaration source exactly
as it would be for an explicitly declared entrypoint.

`./assets/widget.js.map`, `./assets/tokens.json`, and
`./assets/styles.module.css` are recorded in the refusal sidecar's
`inapplicable` array as `non-module-target`. A sourcemap, a JSON document, and a
stylesheet have no ESM runtime surface, so an artifact case over one asserts
nothing about certifiable behavior. It is a census decision, not a refusal, and
it must not suppress the sibling module case or the proposal.

`./assets/empty.js` (zero bytes) and `./assets/comments-only.js` (every byte a
comment) are **refused**, and they are here to pin the one boundary of the
`non-emitting-module-target` disposition that a broken build could otherwise
walk through. Both files emit no JavaScript, and neither is inapplicable: a
module that declares at least one type is a deliberate type module, while a
module with no statements at all is indistinguishable from a publish defect —
`@solid-devtools/shared@0.20.0` ships a zero-byte `dist/index.js` — and nothing
in the bytes tells those apart. `multi-entrypoint`'s `./empty` pins the third
spelling of the same emptiness, `export {}`.

The rule is about *entrypoints only*: an asset an analyzed module imports is
still an ordinary closure member with the role the closure gives it, and
`asset-import` pins that path unchanged. The module extensions this rule
recognizes are exactly the resolver's `RUNTIME_EXTENSIONS` — what the pipeline
already parses as runtime source — so nothing new starts being analyzed.

`unpublished-conditional-target` and `non-emitting-module-target` pin the other
two inapplicable classes. The classes must not overlap: an asset suffix is
answered here and never by the emission premise, which reads statements.
