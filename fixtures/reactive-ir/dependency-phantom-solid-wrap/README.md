# dependency-phantom-solid-wrap

**Claim (ADR 0981).** A package the build reads and finds importing no Solid package is not selected, yet it can return what the program hands it: `wrap` returns its argument, a function that reads the program's own signal. The call of the returned function in a component body is an uncertifiable SC1001 (`package-value-read:untracked`), never silent and never a violation (the package may hand back anything).

`dep-wrap` has one file, `export const wrap = (fn) => fn`, no import, no manifest dependency: selection reads its closure and finds no Solid import, so the package stays off source. `Untracked` calls `g()` in the body (Chrome rc.21: STRICT_READ_UNTRACKED at `g()`, `fixtures/runtime-oracle/phantom-solid-wrap`); `Tracked` reads it in JSX (quiet in Chrome, no row here). What would win the row back is a proof from the package's readable source that a returned value cannot come from a function the program hands it (`docs/open-work.md`).
