# Dependency source (ADR 0284)

Claim: with `--dependency-source dep-hooks` (`.solid-checker/runtime.json`
`dependencySources`), the package's runtime source is analyzed as project code
instead of requiring a contract.

- `LabelNow` (positive): the import binds to the file the package's `exports`
  selects under the `solid` condition, `src/index.jsx`, not the `default`
  build `dist/index.js` (which reads nothing). Its `useLabel` forwards the
  accessor to `readNow` in `src/read.js`, reached by a relative import, which
  calls it: SC1001 at the call.
- `LabelMemo` (negative): the package defers the read into a memo.
- `Badged` (negative): `Badge`'s own untracked read is located in the package,
  so it is not reported.
- `Greeting` (positive): the package's `Object.assign(target, source)` with an
  open source vetoes members only of objects the package creates (ADR 0273's
  dependency internals), so this project's context member still transports.

Without the flag the same sources report SC9011 at `useLabel` and
`useLabelMemo` (no contract describes them) and keep `Greeting`.

`solid-js.d.ts` is `context-member-identity`'s stub (claim-bearing signatures
copied from published 2.0.0-rc.13). `App.tsx` passes `tsc --noEmit`; the
package's JavaScript is not type-checked (`checkJs: false`).
