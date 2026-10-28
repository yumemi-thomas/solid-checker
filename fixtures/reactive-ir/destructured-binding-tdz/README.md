# destructured-binding-tdz

**Claim (ADR 0464).** A binding a destructuring declarator binds (`const [r, setR] = createSignal(0)`, `const [s, setS] = createStore(...)`) is not hoisted: a mention that runs before the declarator throws `ReferenceError` (rc.17 in Chrome: "Cannot access 'r' before initialization", and no Solid diagnostic), and `tsc` is silent when the mention sits in a closure. ADR 0339's proof now decides these mentions too, and a violation at a site holding one it cannot place after the declarator is uncertifiable, whatever the rule.

`Early.tsx` (uncertifiable): a read (`SC1001`), a setter call (`SC2001`), a store read (`SC1001`), a store setter call (`SC2001`) and a store write (`SC2003`), each in an inline `forEach` callback written before the declarator, and a read and a setter call in a hoisted nested declaration that callback calls (the read's propagated origin is the mention), and an identifier bound to `createMemo(...)` read early (ADR 0339 already withheld its call edge; the site is now uncertifiable too).

`Late.tsx` (violations, positive controls, each confirmed by rc.17 in Chrome: `STRICT_READ_UNTRACKED`, `REACTIVE_WRITE_IN_OWNED_SCOPE`, `STORE_WRITE_OUTSIDE_SETTER`): the same mentions after the declarator, the memo read after it, a nested declaration every call of which follows it, and `state.ts`'s exported destructured signal across a one-way import.

Stubs: `createSignal` and `createStore` shapes as loose as the published rc.17 typings (`Store<T> = T`).
