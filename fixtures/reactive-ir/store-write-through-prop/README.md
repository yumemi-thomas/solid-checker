# store-write-through-prop

**Claim (ADR 1172).** ADR 0491 proves a link of a function-local store is still the literal its initializer wrote when nothing can
replace or reach it: the setter never referenced, no proxy leaving as a value. A hand-off by prop to an exact local component (a JSX
tag resolving to a function of the file) now keeps that, in two cases:
- the store handed as `s1={s1}`, where the component reads `props.s1` (or a destructured `s1`, or an exact `const` alias of either) only as the root of a contained use;
- the setter handed as `setS1={setS1}`, where the component never reads that prop.

A write past the root through the prop (ADR 0919's handed route) is then dropped outside the setter: STORE_WRITE_OUTSIDE_SETTER.

| component | outcome |
|---|---|
| `AssignChild` (`props.s1.a.v = 7`, the setter handed and unread), `PushChild` (destructured, `items.push`), `NamespaceChild`, `AliasChild` (`const held = props.s1`) | SC2003 violation (main: uncertifiable `store-write-unproven`) |
| `OrWritesChild` (`props.s1.a.v ||= 7` over `v: 0`: the handed literal's test passes, ADR 1225) | SC2003 violation |
| `AndNoopChild` (`&&=`), `NullishNoopChild` (`??=`) over `v: 0`: no write (ADR 1225) | uncertifiable `store-operation-may-not-write` (main: violations Chrome contradicts) |
| `CallsSetterChild` (the setter read and called), `HandsOnChild` (the prop's link handed on), `WholeChild` (props used whole), `AlsoHandedChild` (the store also leaves the parent) | uncertifiable, as before |

Chrome: `fixtures/runtime-oracle/store-write-through-prop` reports STORE_WRITE_OUTSIDE_SETTER at all seven writes. Stub: rc.21's
`createStore` and `StoreSetter`, `node_modules` from `store-mutators`.

**ADR 1228.** `LetAliasChild` (`r = s1.a; r.v = 7` with `{ s1 }` destructured in the parameter list) and `ParamChild` (`w(s1.a)`, `w` writing its parameter): the flow (ADR 0719) read a render as handing a compiler-built props object, so a sink reading past it ended with no store and no row. It reaches an attribute's value, which a caller may hand a store: SC2003 uncertifiable `unproven-container-root` (main: silent; Chrome reports STORE_WRITE_OUTSIDE_SETTER on the grid programs of these shapes, ADR 1208). Negative: `q`, written only with a fresh object, silent.
