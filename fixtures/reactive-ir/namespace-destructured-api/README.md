# namespace-destructured-api

**Claim (ADR 0458).** `const { onCleanup: cleanup } = S`, with `import * as S from "solid-js"`, binds exactly the export `S.onCleanup` reads, so `cleanup(…)` is the primitive's call wherever `S.onCleanup(…)` would be. Violations (`missing-owner`): `cleanup` in a timer, `readContext` (a string-literal key) in an event handler, `createEffect` (shorthand) in an event handler, and a destructuring written in a component body whose slot runs in a timer.

Silent: the same slots in a component body (owned); a parameter or a local object named `S` (the binder resolves the initializer to it, not to the import); a computed key, a default value, a rest element and a `let` destructuring; an exported slot (other modules mention it). Uncertifiable, never a violation: `lateCleanup` mentioned in `early`, which module code runs before the slot's declarator, so the call throws `ReferenceError` instead of calling `onCleanup` (ADR 0339's proof decides each mention).

tsc: clean against the stub, whose declarations are copied from the published 2.0.0-rc.13 typings (see its header). Runtime: the fuzzer's misuse profile (`--binding destructured`) runs the same shapes on rc.17; the namespace and destructured forms agree program for program.
