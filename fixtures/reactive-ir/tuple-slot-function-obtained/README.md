# tuple-slot-function-obtained

**Claim (ADR 0801).** A program that obtains `Function` has no exact accessor slot, though ADR 0282's census holds: `escaped` hands `globalThis.Function` to a parameter and calls it with a string that patches iteration, which that census does not see (a call of a parameter with a primitive argument). `escaped` and `tracked` are uncertifiable (SC9012, `returned-callable-outside-source-universe`); `body-read` keeps ADR 0222's SC1001 with that SC9012 beside it. Chrome (rc.21): `escaped` warns ASYNC_OUTSIDE_LOADING_BOUNDARY; alone, `body-read` warns STRICT_READ_UNTRACKED and `tracked` stays quiet.

No helper read here: this stub types the accessor as a plain `() => T`, and a helper's read of it is then certified where the real rc.21 typings give `handed-member-read` (invariant 1: a stub looser than the package).
