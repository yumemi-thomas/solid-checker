# parameter-pass-through-dispatch

**Claim (ADR 0213).** In a closed program (a private `package.json`, ADR 0193), a parameter-member obligation whose argument is the enclosing function's own parameter is discharged when that parameter holds a built-in value on every entry: the function is entered only through call expressions (ADR 0203), the parameter is written nowhere, and every call site passes a value `value_origin` proves (ADR 0211, ADR 0212), or a parameter of its own that holds one. A default must hold one too.

| Function | Obligation at `evens(list)` | Why |
| --- | --- | --- |
| `forward` | none | one call passes an array literal; the other, in `twice`, passes `twice`'s parameter, whose only call passes one |
| `withDefault` | none | a default `[]`, an omitted argument and a literal |
| `mixed` | `SC9012` | one call passes `props.items` |
| `rewritten` | `SC9012` | the parameter is assigned |
| `escaping` | `SC9012` | the function is handed to `props.keep` as a value |

Each helper is written inside `App`: a call written in a helper nested in a component keeps its dispatch obligation (ADR 0202), while one in a module-level helper's body raises none. The `SC1001` findings on `props.items` and `props.keep` read in the body are the props rules' and not part of this claim.

The scaffolding (`node_modules`, `solid-js.d.ts`, `tsconfig.json`) is copied from `closed-export-member-dispatch`.
