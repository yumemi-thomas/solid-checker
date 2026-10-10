# value-exports

The tracer for ADR 0099: an export whose value **cannot be invoked** closes
its empty proposable call domains on the producer's stated fact alone.

`FLAG`, `LIMIT`, `NAME`, `OPTIONS`, `SIDES` and `NULLABLE` are value
bindings. The generator proposes their empty call domains exactly as it does
for a function export, and before ADR 0099 every one was withheld as `no
recipe in corpus`: `synthesize` builds a veto only for an export that stated a
call signature, and the census refused the export as
`domain-exhaustiveness … callSignatureNotUnique` -- no body to walk. The
producer now states `notCallableValue` (kind `primitive` for the first three
and `NULLABLE`, `object` for the two literals) beside the declaration with the
single open reason `valueNotCallable`; the certifier closes the domain
vacuously with the site `typefacts-value-export:not-callable:<kind>:<type>`,
and the synthesized veto imports the value and emits `callable-value` if
`typeof` says function after all.

Three exports stay outside the premise, each for its own reason, and the
expected files pin that they stay withheld: `entries` aliases an overloaded
library function (`callSignatureNotUnique`, as before); `parsed` is typed
`any`, which the callability classifier refuses; `Box` carries a construct
signature, and `new Box()` is an invocation. `helper` is an ordinary function
export whose domains close through the implementation census as before.

Types: `index.d.ts` is written by hand and mirrors the runtime; the `solid-js`
stub is the shared rc.3 stub the census fixtures use.
