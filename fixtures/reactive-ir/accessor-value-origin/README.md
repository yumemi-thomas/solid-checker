# accessor-value-origin

**Claim (ADR 0211, phase 2).** An accessor call's value is proven a built-in value when every value the accessor can hold is (`SemanticLookup::accessor_origin`):

- `const [items, setItems] = createSignal(initial)` in a function: the initial value, and every write through the setter, whose every reference in its file is a call's callee. An updater's returns are proven with its previous value assumed built-in;
- `const sorted = createMemo(() => …)`: every value the synchronous compute returns;
- a plain call of an exact project function (`sorted()`, `parse(text)`): every value its synchronous body returns. A parameter it returns proves nothing;
- `a ?? b`, `a || b`, `a && b` and `c ? a : b`: both operands.

Each case reads the accessor in the component body, which is an `SC1001` strict read of its own. The claim is about the `SC9012` dispatch obligation for `evens`'s `filter`, which leaves only where the value is proven.

| Case | Dispatch | Why |
| --- | --- | --- |
| `UpdatedSignal` | none | a literal, a spread of the previous value, a filter of it, `[]` |
| `SortedMemo` | none | `[...items()].sort()` |
| `EscapedSetter` | `SC9012` | the setter is put in an object |
| `StoreWritten` | `SC9012` | a store's array is written |
| `WritableMemo` | `SC9012` | `createSignal(fn)` |
| `StoreMemo` | `SC9012` | the memo returns a store's array |
| `DerivedValues` | none | a derived function, a project helper and a `?? []` fallback over a memo, each returning arrays built here |
| `ReturnedParameter` | `SC9012` | `same` returns its parameter |

`solid-js.d.ts` is `value-origin-member-dispatch`'s with `createMemo`, and with a setter that takes an updater and a `createSignal` that takes a function, as `@solidjs/signals`' do.
