# Dependency source reached through `..` (S3a)

Claim: a dependency source module that imports a sibling directory's module as
`../lib/read.js` is analyzed through that module exactly as one importing
`./read.js` is. The owned WASM check resolved such imports to no source file
until its WASI layer resolved `..` physically (docs/precision-backlog.md, P5
S3a); this fixture pins the native answer both sides must give.

- `App.tsx` `LabelNow` (positive): SC1001 at `useLabel(name)`. The selected
  package's `solid` source (`src/hooks/index.jsx`) reads `name` during the call
  through `../lib/read.js`.
- `App.tsx` `LabelLater` (negative): `useLabelMemo` reads through the same
  module inside `createMemo`, so nothing is read during the call. No finding.

The fixture holds no symbolic link: a committed one would be this repository's
first, and the link shapes live in the WASM parity gate's generated trees.

`solid-js.d.ts` is `dependency-source-program`'s stub, byte for byte
(claim-bearing signatures copied from published 2.0.0-rc.13). The sources pass
`tsc --noEmit` against the stub and against the published rc.13 types. The
unresolved case, the same tree with `src/lib/read.js` removed, is a refusal,
so it is a WASM parity gate case rather than a coverage fixture.
