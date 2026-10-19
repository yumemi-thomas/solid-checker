# Dependency source reached through `..` (S3a)

A dependency source module importing `../lib/read.js` is analyzed through that module exactly as one importing `./read.js` is (the WASM check once resolved no source file until its WASI layer resolved `..` physically): `LabelNow` is SC1001 at `useLabel(name)`, `LabelLater` (`useLabelMemo` in `createMemo`) none. No symbolic link is committed; the unresolved variant (`src/lib/read.js` removed) is a WASM parity gate refusal case. `solid-js.d.ts` is `dependency-source-program`'s (published rc.13 signatures); `tsc` clean.
