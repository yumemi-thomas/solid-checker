# parameter-copy-alias

**Claim (ADR 1233).** A `const` copy of a parameter nothing writes (here a destructured prop slot) is an exact alias of it (ADR 0463), as a copy of a `const` is: `Copied` (the copy called in an effect's apply) is an SC1001 violation (main: SC9012 uncertifiable `reactive-dispatch-unresolved`; Chrome rc.21 STRICT_READ_UNTRACKED on the grid's `alias`/`prop-destructured` programs). Uncertifiable: `Written`. Clean: `InHandler`. Stub: `effect-cleanup-disposal-only`'s.
