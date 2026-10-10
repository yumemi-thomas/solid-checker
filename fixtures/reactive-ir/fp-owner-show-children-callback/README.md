# fp-owner-show-children-callback

**Claim.** Only the function that *is* a control-flow component's children
value has accessor parameters: the sole `{...}` child expression, or a
`children={...}` attribute. Any other function written inside the element is a
callback of its own element and its first parameter is whatever that callback's
caller passes.

Before, source discovery took every outermost function inside a `<Show>`
element as its children callback, so the raw `<For>` row item, the `Event` of
an input handler, and the element a `ref` callback receives were all
registered as accessors. Template-literal uses of those plain values were then
reported as `uncalled-accessor`, and writes to them as `no-direct-mutation`.

| Case | Finding | Why |
| --- | --- | --- |
| `<Show>{(user) => ...${user}...}</Show>` | `uncalled-accessor` | non-keyed `Show` hands an accessor |
| the same through `children={...}` | `uncalled-accessor` | the `children` attribute is the children value |
| default-keyed `<For>` item, `onChange` event, `ref` element under `<Show>` | none | plain values: rc.9 `For` passes the raw item, handlers get an `Event` |
| `<For>`'s second parameter under `<Show>` | `uncalled-accessor` | the index is an accessor |

**Stub.** `solid-js.d.ts` copies every declaration a claim rests on verbatim from
the rc.9 typings (header lists them); the file also type-checks against the
real installed `solid-js@2.0.0-rc.9` and `@solidjs/web@2.0.0-rc.9` (`tsc
--noEmit`, `strict`, `jsxImportSource: "@solidjs/web"`, `skipLibCheck`).
