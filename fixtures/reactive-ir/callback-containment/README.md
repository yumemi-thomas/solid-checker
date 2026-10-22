# callback-containment

`missing-owner` (SC4001) for the callback an event handler or a primitive's argument names. The callback is the function literal the argument is (parentheses and TypeScript wrappers removed), or the function a bare identifier names. An event handler's tuple `[handler, data]` names its element 0, the one documented exception (`review-r7-event-array`). Any other expression hands over a value: a call, an IIFE, or `untrack(() => …)`. A function literal inside that value is not the callback, so it is not an unowned callback (SC4001 review D).

Positive controls, each a violation:

- `IdentifierHandler` (line 7): a bare identifier names its function, which the DOM calls with no owner.
- `LiteralHandler` (line 14): a function literal written as the handler.
- `TupleHandler` (line 19): element 0 of a handler tuple.
- `NamedApply` (line 23): a named apply callback runs from the flush with no owner.

Negatives, with no SC4001. Each of these reported SC4001 before the fix with no runtime diagnostic:

- `IifeHandler` (line 31): an IIFE is the handler's value, evaluated while the component renders.
- `UntrackHandler` (line 36): `untrack` runs its callback during the call, under the component's owner.
- `FactoryHandler` (line 41): a factory the handler calls runs its body while the component renders.
- `IifeApply` (line 50) and `FactoryApply` (line 56): an IIFE or factory result is an effect's apply callback, evaluated while the effect is created.

The call-valued effect functions on lines 50 and 60 also report SC7001 (missing-effect-function, uncertifiable): a separate rule about the effect function, not this fixture's claim. The call-valued cases lose their SC4001 recall: docs/open-work.md records the resolver change that could recover them.

Declaration stubs copy the `solid-js` declarations of `fp-owner-show-children-callback` (published 2.0.0-rc.9 typings), as `ref-expression-owner-walk` does, with `createRoot`, `runWithOwner` and `untrack` from `@solidjs/signals` 2.0.0-rc.13. The event handler union comes from `@solidjs/web` 2.0.0-rc.13 `types/jsx.d.ts` (`EventHandler`, `BoundEventHandler`, `EventHandlerUnion`); the `target` of `EventHandler` is loosened to `unknown`, since no handler here reads it.
