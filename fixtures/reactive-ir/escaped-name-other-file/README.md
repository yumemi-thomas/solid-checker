# escaped-name-other-file

ADR 1152: the veto is the program's, not the file's. `App.tsx` writes no escape and `set(2)` in `BodyWrite` is a write during render; `names.ts` writes the identifier `\u0061lias` with an escape. A file's names reach the others through the summaries, the call graph and the component test, so the violation of `App.tsx` is uncertifiable (`escaped-name-unresolved`) and both files carry the notice that spans them.

The twin is `escaped-name-plain-string`, where no file writes a name with an escape and the SC2001 stands.

The stub reduces the published rc.21 declarations (`createSignal`). Package bytes are not copied here.
