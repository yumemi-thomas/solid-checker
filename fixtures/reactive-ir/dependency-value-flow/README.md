# dependency-value-flow

**Claim (ADR 0998).** A value a package the build does not read as source returned is read as possibly reactive on every route to its read. ADR 0981's rule
followed a read back through a call, a member, an operand and a plain binding; a value that reached its read through an element of a literal, an assignment to
an existing binding, a parameter of a project function or a copy had no row, and Chrome rc.21 warns STRICT_READ_UNTRACKED there all the same.

`dep-router` is the package of `dependency-phantom-solid-undecided` (its closure cannot be read, so it is never selected); `useLocation()` returns getters over
a memo. Cells:

- `Direct`: ADR 0981's row (`package-value-read:untracked`);
- `ViaArray`, `ViaLet`, `ViaObject`, `ViaParam`, `ViaCopy`: the forward walk's rows (`package-value-read:flowed`), uncertifiable, never a violation;
- `ViaProp`, `ViaPropPattern`, `ViaContext` (ADR 1002): the value handed on as a JSX attribute, read untracked by the project component the tag names (its
  `props.location`, its pattern's slot) or by a `useContext` of the provider: the same row (`package-value-read:flowed`);
- `TrackedObject`, `TrackedLet`, `ViaPropTracked`: the same routes read in a tracked expression: no row;
- `Local`: a project function named `useLocation`: no package, no row.

Chrome: `fixtures/runtime-oracle/package-value-flow`.
