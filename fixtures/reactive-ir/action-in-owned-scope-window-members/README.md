# action-in-owned-scope-window-members

**Claim (ADR 1108, the named premise of ADR 0974 "the global object reached through a DOM object's typed member").** The DOM types an event's `view` (`UIEvent.view: Window | null`) and a message's `source` (`MessageEventSource`, which holds `WindowProxy`) as this realm's window. `module_reach.rs` takes either member, read off any object the name walk does not follow, for the global object, as it takes `defaultView` (ADR 0974): a member written through it writes that global, and the value handed on is an escape (a writer under any name). A call of a global some package the program loads writes that way, where the writer's closure may reach a Solid action, is an uncertifiable site (`package-may-call-action:reaches-solid`). Never a violation.

- **Uncertifiable**: `WrittenThroughEventView` (`btoa`, `fz-wview`: `event.view.btoa = …` in a click listener the module fires as it loads), `WrittenThroughMessageSource` (`atob`, `fz-wsource`: `message.source.atob = …`, a message dispatched synchronously with `source: window`).
- **Uncertifiable, unchanged (ADR 0974)**: `WrittenThroughDefaultView` (`unescape`, `fz-wdocview`: `document.documentElement.ownerDocument.defaultView.unescape = …`).
- **Silent**: `WrittenByClearPackage` (`fz-wviewclear` writes `escape` through `event.view` and names no Solid package), `Unwritten` (`encodeURIComponent`) and `WrittenInHandler` (`btoa` called in a click handler).

Chrome (rc.21, runtime oracle case `fixtures/runtime-oracle/package-window-members`, and the pattern lane's `pkgaction/global-route-event-view`): `ACTION_CALLED_IN_OWNED_SCOPE` at each uncertifiable call; the silent components are quiet.

Not followed (`docs/open-work.md`): members typed `EventTarget` that are the window only when a listener is on it (`event.target`, `currentTarget`, `relatedTarget`, `composedPath()`), `this` in a listener, `window.opener` (another realm's window, typed `any`), and a frame's `contentWindow` (another realm).

`node_modules/fz-*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-global-writes`; `.solid-checker/runtime.json` closes the program boundary.
