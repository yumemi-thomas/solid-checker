# host-callback-read-timing

**Claim.** A read in a callback a host API retains and may invoke on its
invoker's stack, written in a component body, is **uncertifiable** under
`SC1001` (and a pending async read there under `SC5001`), never a proven
violation. The host may invoke the callback inside the component body's
strict-read window, or after it, and nothing in the facts proves which
(`execution_role::host_callback_timing`,
`runtime_semantics::runs_on_invoker_stack`).

The callbacks are the non-fresh-stack remainder of the runtime table's
deferred callbacks, matched by the compiler-selected standard-library
declaration, never by spelling: argument 1 of every default-library
`addEventListener` declaration (`EventTarget`, `Window`, and each DOM subtype
that redeclares it, such as `HTMLElement`, `HTMLInputElement`, `Document`),
the bound arguments of `Function.prototype.bind`, the callbacks of
`PromiseLike.then`, and the Geolocation `getCurrentPosition`/`watchPosition`
callbacks. The walk from the read outwards passes through standard-library
inline callbacks (`list.forEach`) and ends at any other call.

Measured on the published bytes, `solid-js`/`@solidjs/signals` `2.0.0-rc.3`
and `2.0.0-rc.9`, dev and prod builds, loaded through an import map in headless
Chrome, each case mounted as a component under `createRoot`:

- a signal read in a listener dispatched with `dispatchEvent` or clicked with
  `el.click()` in the body, in a bound function called in the body, and in a
  synchronous thenable's callback raises `STRICT_READ_UNTRACKED` (dev); the same
  callbacks dispatched after mount, or run by a real promise, raise nothing;
- a pending async read in a listener dispatched in the body, or in a
  synchronous thenable's callback, throws `PENDING_ASYNC_UNTRACKED_READ` (dev);
  dispatched later, or through a real promise, it throws a plain
  `NotReadyError`;
- Chrome ran neither Geolocation callback inside the body: from an active
  document the error callback arrived asynchronously, and from a detached
  iframe's document neither callback ran at all. The specification's "call back
  with error" for a document that is not fully active runs the callback inside
  the call, so Geolocation is treated as the other three.

| Case | Finding | Why |
| --- | --- | --- |
| a read in the component body | `SC1001` violation | the strict-read window is open |
| a read in an `addEventListener` listener on an `HTMLElement`, `HTMLInputElement`, `EventTarget`, `window`, `document` | `SC1001` uncertifiable | the listener may be dispatched inside the window or after it |
| a read in a `PromiseLike.then` callback, a bound argument, a Geolocation callback | `SC1001` uncertifiable | the same: each may run on its invoker's stack |
| a read in a `forEach` callback in a listener; a read through a local helper called in a listener | `SC1001` uncertifiable | an inline callback and a helper call run where the listener does |
| a listener the body dispatches synchronously right after registering it | `SC1001` uncertifiable, **not proven** | it does run inside the window, but the checker does not prove a dispatch reaches a registered listener (same target, same event type, no removal, on every path) |
| a pending async read in a listener | `SC5001` uncertifiable | `PENDING_ASYNC_UNTRACKED_READ` inside the window, `NotReadyError` after it |
| a read in a `setTimeout` callback | none | a fresh-stack scheduler runs it after the body returned (`fresh-stack-scheduler-read-role`) |
| a read in a JSX `onClick` handler | none | the compiler's event callback |
| a read in a listener handed to a project object's `addEventListener` method, which calls it at once | `SC1001` violation | not the default-library declaration: it runs the listener in the body |

**Stub.** `solid-js.d.ts` is `fresh-stack-scheduler-read-role`'s, byte for
byte; the claims rest on `createSignal` (the reads) and `createMemo` (the async
source), verbatim from the published rc.3 typings as its header describes. The
DOM declarations are TypeScript's own `lib.dom.d.ts`. `App.tsx` type-checks
cleanly against the stub and against the real `solid-js`/`@solidjs/signals`/
`@solidjs/web` rc.3 and rc.9 installs (`tsc --noEmit`, TypeScript 5.9.3,
`strict`, `jsxImportSource: "@solidjs/web"`), and the checker reports the same
findings against all three.
