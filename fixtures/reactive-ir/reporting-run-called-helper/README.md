# reporting-run-called-helper

**Claim (ADR 1189).** ADR 1166's reporting run gives a read in a named callback the role of code at the callback's run. That role is
proved only when the function holding the run gets its role from no function that may not run as a component: it is a primitive's
callback literal, or every reference of it is its name, an export, or a direct call written outside every undecided component region in
a function that qualifies in turn (`named_callbacks::declarer_role_is_its_own`). Otherwise the read stays uncertifiable.

| component | outcome | Chrome |
|---|---|---|
| `MappedRow` (the census false positive: `wrapUse` called from `Row`, which `[1].map(Row)` runs inside a JSX expression) | uncertifiable (main ff7f6cf0c: SC1001 a violation) | silent |
| `RowDeclared` (the callback declared and run in `Row` itself) | uncertifiable | silent |
| `BodyCalledHelper`, `DeclaredHelper`, `InvokedHelper` (a helper the body calls directly; a function declaration; an immediately invoked arrow) | SC1001 a violation | STRICT_READ_UNTRACKED |
| `BodyDeclared` (declared in the component body) | SC1001 a violation | STRICT_READ_UNTRACKED |

Chrome: `fixtures/runtime-oracle/reporting-run-called-helper`. Stubs: `array-callback-store-read`'s.
