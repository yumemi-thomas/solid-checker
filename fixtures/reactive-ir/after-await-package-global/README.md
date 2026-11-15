# after-await-package-global

**Claim (ADR 0999).** A call of a standard-library declaration after an `await` ends in `library-runs-nothing` (ADR 0976) only when no installed package the
program loads writes the global the call goes through. The declaration says what the library's function does; a package that replaces the global
(`window.structuredClone = …`) runs its own, which may call a function the program handed it earlier.

- `Replaced`: `structuredClone({ n: 1 })` after the await is an uncertifiable SC1002 (`read-after-await-unaccounted`): `fz-mock` writes `structuredClone`.
  It was silent, and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT (the package calls the accessor `mock(flag)` handed it).
- `Untouched`: `JSON.stringify` and `Math.max` after the await keep the reason: no row.

`node_modules/fz-mock` is **not** published bytes: it is written for this fixture and names no Solid package (every writer counts, not only one that reaches a
Solid action). Chrome: `fixtures/runtime-oracle/after-await-package-global`.
