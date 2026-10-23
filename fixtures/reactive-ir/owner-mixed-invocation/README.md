# owner-mixed-invocation

**Claim (ADR 0363).** A callback the host runs as a proven event handler (an intrinsic element's `on…` attribute or an exact spread key, ADR 0319's walk) and that something else also runs (a `runWithOwner` slot, a plain call in owned setup) has no owner on the event run. An `onCleanup` or effect reached on that run is a proven `SC4001` violation, worded "when run as the `onClick` handler of `<button>`"; the owned runs are not asked. The premise is ADR 0327's (an event handler fires with no owner) with ADR 0356/0361's veto: a synchronous dispatch that fires the type of an attribute the callback's event runs end at keeps it an obligation (ADR 0361, amended).

`App.tsx` (rc.17 stubs, byte-faithful declarations in `solid-js.d.ts`):

| export | shape | result |
| --- | --- | --- |
| `RunWithOwnerAndHandler` | `runWithOwner(owner, h.onClick)` and `<button {...h}/>` (runtime oracle `review-r1-owner-mix`) | violation |
| `OwnedRunAndHandler` | `run()` calls the callback in setup, `{ onClick: run }` is spread (oracle `review-r3-owner-double-call`) | violation |
| `PlainCallAndHandler` | `cb()` in the body and `onClick={cb}` | violation (ADR 0327's compiler edge, unchanged) |
| `PlainCallAndSpreadHolder` | `cb()` in the body and `{ onClick: cb }` spread | violation |
| `HelperOnEventPath` | the mixed callback calls a helper that registers the cleanup | violation in the helper |
| `ClosureRoot` | `createRoot(() => ({ onClick: cb }))`: the root never invokes `cb` (oracle `review-r1-owner-mix` :13) | violation (ADR 0363 root pass-through; no owned run) |
| `RunWithOwnerOnly` | `runWithOwner(owner, cb)` only | no violation: no event edge |
| `UnresolvedHolder` | `registry.onClick = cb`, a holder the walk cannot follow | obligation |
| `WriteInMixedCallback` | a setter in the mixed callback | SC2001 obligation, unchanged: the write is legal on the owned run |
| `RootInsideMixedCallback` | `createRoot(() => onCleanup(…))` in the mixed callback | no finding: the root owns it on every run |
| `GuardedCleanup` | `if (getOwner()) onCleanup(…)` | no finding |
| `LaterAttributeOverride` | `cb` also in `runWithOwner`; `<button {...h} onClick={…}>`: the later attribute replaces the spread key (ADR 0364), so `cb` is never the click handler | obligation |
| `LaterSpreadUnknown` | the same with a later `{...props.rest}` whose keys are unknown | obligation |
| `LaterSpreadExactWithKey` | the same with a later exact `{...{ onClick: … }}` | obligation |
| `LaterSpreadExactWithoutKey` | a later exact holder with only `onScroll` leaves `onClick` alone | violation |
| `AttributeBeforeSpread` | `onClick={…}` BEFORE `{...h}`: the holder is the later source | violation |
| `ComponentSpreadOverride` | a component spreads its props onto a `<button>` and then sets its own `onClick` | obligation |
| `EventOnlyOverride` | the event-only analogue (`cb` has no other invocation): no unowned proof; the spread holder is also unresolved here (`registry.onClick` is written project-wide, ADR 0299), so the obligation is the spread's | no `onCleanup` finding, spread obligation |
| `AttributeThenSpread` | the compiler's own `EventHandler` role: `onClick={cb}` then `{...{ onClick: … }}`; lowered into the same sources array, the later spread wins | obligation |
| `AttributeThenSpreadWithoutKey` | the same attribute with a later exact holder that has no `onClick` | violation |
| `InlineThenUnknownSpread` | an inline handler literal followed by `{...props.rest}` | obligation |

Variants (each a project; the root's `package.json` closes the program):

| variant | what it pins |
| --- | --- |
| `sync-dispatch` | a `dispatchEvent` of `ping` on a parameter receiver: it fires `ping` only, and the mix's event run ends at `onClick` (ADR 0361, amended), so the event run is still a violation |
| `sync-dispatch-click` | the same with `new Event("click")`: the click handler may run on the dispatcher's stack (ADR 0356), so the mix keeps its obligation |
| `window-dispatch` | a dispatch that reaches no element handler (ADR 0361) withholds nothing: the event run is still a violation |
| `may-resume` | the cleanup (or a helper call) follows a skippable `await` (ADR 0357): the possible-suspension edge ends the event path, so it is an obligation |
| `exact-holder` | event-only handlers in function-local holders the checker resolves exactly (the main project's `registry.onClick` write makes `onClick` holders unresolved, ADR 0299): the graph edge is proven unowned unless a later `onClick` attribute, a later exact holder with the key or a later unknown spread follows (obligations); a later exact holder with `onScroll` only, and an earlier attribute, keep the violation |

`ClosureRoot` is the one shape with no owned run: the event-only walk (ADR 0327) now passes through the callback of an owner-supplying call (`createRoot`, `runWithOwner`) that returns the holder, since the callback only returns the value and the call is the thing the walk follows.
