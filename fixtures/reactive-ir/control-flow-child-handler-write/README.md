# control-flow-child-handler-write

**Claim.** A function written inside an element that is itself a child of a
control-flow component (`<Show>`, `<For>`, `<Repeat>`, `<Match>`, `<Switch>`)
belongs to that element, not to the control-flow component. An event handler
there is the compiler's event callback, and a signal write in it is legal
(`SC2001` stays silent). Only a function written at the control-flow
component's own level -- its render callback, or code run inline from its
children -- takes the component's render role
(`execution_role::control_flow_execution_role`).

Before, the classifier took the *outermost function anywhere inside* the
`<Show>` element as its render callback, so `<Show><div onClick={() =>
set(x)} /></Show>` reported the handler's write as a write in the owned scope
of the enclosing component. The same `<div>` outside `<Show>` was silent.
`@kobalte/core`'s `resizable` handle had six of these.

Measured on the published `solid-js`/`@solidjs/signals`/`@solidjs/web`
`2.0.0-rc.9` bytes (byte-identical to the audited archives), compiled with
`babel-preset-solid@2.0.0-rc.2` (`@dom-expressions/babel-plugin-jsx@0.50.0-next.44`)
and run under jsdom, dev and prod builds, each component mounted with
`render(() => createComponent(...))` and every `id`'d element sent `mouseenter`
and `click`:

- the handlers in `HandlerInShow`, `HandlerInNamespaceShow` and
  `HandlerInRenderCallback` write and raise nothing, dev or prod;
- `WriteInShowChildren` and `WriteInRenderCallback` throw
  `REACTIVE_WRITE_IN_OWNED_SCOPE` while mounting in dev (prod has no guard).

(The probe compiled with `delegateEvents: false`: the published compiler's
delegated `$$click` key is not the `_$$click` key `@solidjs/web@2.0.0-rc.9`
reads, so a delegated click never reaches its handler under that pairing.
Direct listeners test the same question -- the owner a handler runs under.)

| Case | Finding | Why |
| --- | --- | --- |
| `onMouseEnter`/`onClick` on a `<div>` written as a `<Show>` child | none | the element's event callback, run when the event fires |
| the same under `Solid.Show` (namespace import) | none | the same element-level callback |
| `onClick` on an element the render callback returns | none | unchanged: nested in the render callback, a deferred callback |
| a write in an IIFE written as the `<Show>` child | `SC2001` violation | runs while the children render, under `<Show>`'s owner |
| a write in the render callback's own body | `SC2001` violation | runs under `<Show>`'s owner |

**Stub.** `solid-js.d.ts` holds `createSignal` and the two non-keyed `Show`
overloads verbatim from the rc.9 typings, as its header lists. `App.tsx`
type-checks cleanly against the stub and against the real rc.9 installs
(`tsc --noEmit`, TypeScript 5.9.3, `strict`, `jsxImportSource:
"@solidjs/web"`, `skipLibCheck` because `solid-js@2.0.0-rc.9`'s own
`types/index.d.ts` fails to resolve five of its re-exports).
