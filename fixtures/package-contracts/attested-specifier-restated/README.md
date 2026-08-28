# Resolved CJS spelling enters exact closure

`index.js` names `./impl.cjs` while the checked source is `impl.cts`.
TypeScript's exact module resolution establishes that substitution. The
temporary-v2 artifact record therefore includes the resolved file in its
closure digest instead of guessing from an extension table or retaining a
human review note.

The proposal keeps unrelated unresolved semantic domains open and records four
closure candidates. A resolver failure would refuse or open only the affected
artifact/claims; it would not silently hash `index.js` alone.
