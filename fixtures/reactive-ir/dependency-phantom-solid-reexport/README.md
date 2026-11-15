# dependency-phantom-solid-reexport

**Claim (ADR 0981).** A package that only re-exports a phantom package is a Solid package for selection too: the project imports `dep-shell`, which declares nothing and imports no Solid package itself, and re-exports `dep-router`, which imports `solid-js` and declares nothing.

The scan follows the closure's imports of other installed packages, each read the same way, so `dep-shell` is selected, and `dep-router` after it. SC1001 violation at `location.search`. Chrome (`fixtures/runtime-oracle/phantom-solid-reexport`, selection pinned): STRICT_READ_UNTRACKED.
