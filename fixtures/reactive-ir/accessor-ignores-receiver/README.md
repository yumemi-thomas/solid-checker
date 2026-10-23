# accessor-ignores-receiver

**Claim (ADR 0319).** A Solid primitive's returned function held as an object-literal member ignores the receiver it is called through, so ADR 0319's walk keeps following a callback through that object. That covers a `createSignal` slot, a `createMemo` result and a `createStore` setter bound by `const` directly from the call. rc.17 builds the accessors and signal setters with `read.bind(null, node)` / `setSignal.bind(null, node)`, and the store setter is an arrow.

`mutation.ts` copies kui's `createMutation`: `call` runs in the returned async `run` before its first await, and callers call `handle.state()` beside `handle.run()`.

- Quiet: `Save` (the callback's read runs in the click handler, legal), `SaveMemo` (setter and memo members), `SaveStore` (store setter member), `SaveWrapped` (an arrow wrapper, already proven), `SaveAfterAwait` (the callback's own read after its await has no window, ADR 0323).
- `SC1001` uncertifiable: `SaveThis` (a method that reads `this`), `SaveBound` (`state.bind(null)`), `SaveHeld` (a held tuple destructured later), `SaveOpaque` (an unresolved factory), `SaveShadowed` (a local `createSignal`), `SaveLate` (`run` calls the callback after its own await: a continuation no invocation places), `SaveInBody` (`run` called in the component body: ADR 0319 places only non-reporting roles, so the read is not claimed).
- `SC2001` uncertifiable at `setStore` in `createStoreMutation`'s `run`: write placement never asks ADR 0319.

Variants `bind-replaced` (a visible `Function.prototype.bind` write) and `iteration-replaced` (a visible `Array.prototype[Symbol.iterator]` write, ADR 0349) withdraw the premise: `Save`'s read is an `SC1001` obligation there.

Stub: the published rc.17 `solid-js` and `@solidjs/signals` declarations, copied from `await-continuation-reads`.
