# invocation-flow-owner

**Claim (ADR 0327).** A function literal whose every invocation, followed
forward as ADR 0319 follows it, is a host event dispatch runs with no owner:
an `onCleanup` or effect created there is a proven `SC4001`, exactly as when
the literal is written on the element. The owner walk is stricter than the
read walk: an invocation through any primitive's callback slot refuses, and a
call's role comes only from the walk itself or a compiler event-handler fact,
because `createRoot` and `runWithOwner` pass their caller's *read* role
through while supplying an owner.

| Case | Finding | Why |
| --- | --- | --- |
| `Spread` | `SC4001` violation ×2 | the callback runs from the `onClick` `handlers` returns, spread onto a button |
| `Attribute` | `SC4001` violation | the returned handler is given as `onClick={h.onClick}` |
| `Imported` | `SC4001` violation | the helper is imported from `lib.ts` |
| `UnderOwner` | none | the handler runs the callback under `runWithOwner(getOwner(), …)` |
| `UnderRoot` | none | the handler runs the callback in `createRoot` |
| `AlsoInBody` | none (not proven) | the handler is also called in the component body, which has an owner; mixed invocations refuse |
| `InClassField` | none | the call is a class field initializer, constructed inside `createRoot`; a call inside a class refuses the walk |

The runtime confirms the positives and the two owned controls:
`fixtures/runtime-oracle/oracle-owner-flow`, `oracle-owner-flow-import` and
`oracle-owner-flow-owned`.

`solid-js.d.ts` is `invocation-flow`'s stub plus `runWithOwner`, `getOwner`
and `createRoot` copied from `@solidjs/signals@2.0.0-rc.13`
`dist/types/core/owner.d.ts`; `Owner` keeps two of its published members, and
no claim reads its shape.
