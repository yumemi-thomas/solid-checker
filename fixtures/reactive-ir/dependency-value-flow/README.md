# dependency-value-flow

**Claim (ADR 0998).** A value a package the build does not read as source returned is read as possibly reactive on every route to its read. ADR 0981's rule
followed a read back through a call, a member, an operand and a plain binding; a value that reached its read through an element of a literal, an assignment to
an existing binding, a parameter of a project function or a copy had no row, and Chrome rc.21 warns STRICT_READ_UNTRACKED there all the same.

`dep-router` is the package of `dependency-phantom-solid-undecided` (its closure cannot be read, so it is never selected); `useLocation()` returns getters over
a memo. Cells:

- `Direct`: ADR 0981's row (`package-value-read:untracked`);
- `ViaArray`, `ViaLet`, `ViaObject`, `ViaParam`, `ViaCopy`: the forward walk's rows (`package-value-read:flowed`), uncertifiable, never a violation;
- `TrackedObject`, `TrackedLet`: the same routes read in a tracked expression: no row;
- `Local`: a project function named `useLocation`: no package, no row.

Chrome: `fixtures/runtime-oracle/package-value-flow`.
