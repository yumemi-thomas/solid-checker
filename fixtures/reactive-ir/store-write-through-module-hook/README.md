# store-write-through-module-hook

**Claim (ADR 1232).** A store a hook of another module creates (`hooks.ts`: `return { s1, setS1 }`) is proved in the hook's file:
the hook's every reference is a call the call graph resolved (or its declaration, a type query), and each call's value is carried in
its own file (ADR 1231's `carried_uses`) to uses that are contained (`contained_store_use_in`, the literal read in `hooks.ts`), the
setter's slot reaching none. A write past the root is then dropped outside the setter, judged against the hook's literal
(`links_wrappable_across`).

Violations (Chrome STORE_WRITE_OUTSIDE_SETTER, `runtime-oracle/store-write-through-module-hook`): `Destructured`, `Member`,
`Shared` (a module-scope call, an array mutator). On main they are uncertifiable.

Uncertifiable: `Kept` (the hook's value is handed to a function), `SetterRead` (the setter is called).

Stub: `store-write-through-carrier`'s.
