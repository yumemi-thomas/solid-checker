# fp-exec-control-flow-callback

**Claim.** Only a function written in a control-flow component's *children* is
its render callback (`execution_role::control_flow_execution_role`,
`named_callback_roles`). A helper that is merely *mentioned* in `when`, `each`,
`count`, a `fallback` or a nested element's attribute is not passed to the
component as a callback, and a function written in an attribute (the predicate
in `each={items.filter(...)}`) is an ordinary expression of that attribute.
Reads inside them stay with the roles their real call sites prove, which for
these cases is a tracked JSX attribute: no `SC1001`.

Before, three things placed such code in the render callback's role, an
untracked rendering one:

- `named_callback_roles` admitted every identifier that referred to a local
  function anywhere inside a `<Show>`/`<For>`/`<Match>`/`<Repeat>` element
  (`when={visible() > 0}` admitted `visible`), so the helper's own body read was
  reported as a read in an untracked render callback;
- `control_flow_execution_role` took the outermost function *anywhere* inside
  the element, including one inside an attribute, as the callback;
- a read in a JSX fragment returned by the callback (`<>{tone().length}</>`) was
  not recognised as tracked, because a fragment has no element fact.

Runtime ground truth (2.0.0-rc.9, dev, from the defect triage in
`sc-feedback/rust/target/defect-sweep/triage-strict.md`): the strict-read
window is opened only by a component body, an effect apply, a `<For>`/`<Repeat>`
callback, the `<Show>` function child and `untrack(fn, label)`; a helper called
from `when`, `each` or `count` runs inside a compiled computation, which closes
it.

| Case | Finding | Why |
| --- | --- | --- |
| `visible()` helper in `when`, `when` with `keyed`/`fallback`, `each`, `count`, `<Match when>`, a child element's attribute | none | the helper is only referenced from a tracked attribute |
| `props.items.filter((p) => p.slug !== props.id)` in `each`, `Array.from({ length: n() }, ...)` in `each`, `props.items.some(...)` in `when` | none | the arrow belongs to the attribute, which is tracked |
| `<>{tone().length}</>` in a `<Show>`/`<For>` callback | none | the fragment's expression is compiled into its own insert effect |
| `const now = n()` in a component body | `SC1001` violation | the component body is not a tracking scope |
| `props.variant ?? "primary"` in an exported component with no call site in the project | `SC1001` uncertifiable | the component body is not a tracking scope, but the call sites cannot be enumerated (unchanged) |
| `{(tone) => { const snapshot = n(); ... }}` in `<Show>` / `<For>` | `SC1001` violation | the callback body is not a tracking scope |
| `const renderItem = (item) => { const snapshot = n(); ... }` used as `<For>{renderItem}</For>` | `SC1001` violation | a named render callback is still the render callback |

**Stub.** `solid-js.d.ts` copies `createSignal` (plain-value overload),
`createMemo` and the `For`, `Repeat`, `Show`, `Switch`, `Match` declarations
with their helper types from the published rc.9 typings; only the global JSX
namespace is spelled out locally so the fixture type-checks without
`@solidjs/web`. Every signature a claim here depends on is byte-faithful.
`App.tsx` passes `tsc --noEmit` (TypeScript 5.9.3, `strict`) against this stub
and against the real rc.9 install with `jsxImportSource: "@solidjs/web"`.
