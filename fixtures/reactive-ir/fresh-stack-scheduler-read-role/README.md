# fresh-stack-scheduler-read-role

**Claim.** A read in a callback handed directly to a reviewed fresh-stack host
scheduler does not run in the component body that scheduled it, so it is not a
strict-read violation (`SC1001`). Its execution role is
`ExecutionRole::DeferredCallback`, the role a deferred primitive position gets:
after the scheduling call returns, outside its tracking pass
(`execution_role::fresh_stack_callback_role`).

The schedulers are the analyzer's `FRESH_STACK_SCHEDULERS`
(`rust/crates/solid-reactive-ir/src/runtime_semantics.rs`), matched by the
compiler-selected standard-library declaration, never by spelling. Each runs
its callback from a task or microtask queue on an otherwise empty stack, so the
component body's strict-read label, listener and owner are all gone.

Measured on the published bytes, `solid-js`/`@solidjs/signals` `2.0.0-rc.3`
(the kobalte sweep install) and `2.0.0-rc.9`, dev and prod builds, loaded
through an import map in headless Chrome, each case mounted as a component
under `createRoot`:

- a read in the component body raises `STRICT_READ_UNTRACKED` (dev); the same
  read in a `setTimeout`, `setInterval`, `queueMicrotask`, `Promise.then`,
  `requestAnimationFrame`, `requestIdleCallback` or `MutationObserver` callback
  scheduled from that body does not;
- `getObserver()` and `getOwner()` are `null` in the callback, and the callback
  runs once however often the signal it read is written afterwards: the read
  subscribes to nothing, so there is no tracking premise to report either;
- a write or an action in a `setTimeout` or `queueMicrotask` callback raises
  nothing, where the same write in the body raises
  `REACTIVE_WRITE_IN_OWNED_SCOPE` and the action `ACTION_CALLED_IN_OWNED_SCOPE`
  (dev);
- a pending async read there does not raise `PENDING_ASYNC_UNTRACKED_READ`, as
  the body read does (dev). A source that has settled once serves its settled
  value during a re-ask; one that never settled throws a plain `NotReadyError`,
  in dev and prod, exactly as it does in an `addEventListener` listener
  dispatched after mount.

The other deferring host callbacks the runtime table knows (`addEventListener`,
`Function.prototype.bind`'s bound arguments, `PromiseLike.then`) were probed the
same way and are **not** given the role: each runs on its invoker's stack. A
listener dispatched with `dispatchEvent` or `el.click()` in the component body,
a bound function called in the body, and a synchronous thenable all run the read
inside the strict-read window and raise `STRICT_READ_UNTRACKED`; dispatched
later, or through a real promise, they do not. Registration alone does not
decide which, so those reads keep their previous role.

| Case | Finding | Why |
| --- | --- | --- |
| a read in the component body | `SC1001` violation | the strict-read window is open |
| a read in a `setTimeout`, `queueMicrotask`, `Promise.then`, `requestAnimationFrame` callback | none | the callback runs on a fresh stack after the body returned |
| a read in a `forEach` callback inside a `setTimeout` callback | none | a standard-library inline callback runs where its call does |
| a read through a local function called in a `setTimeout` callback | none | the call runs in the timer callback |
| a timer callback that reads a signal and writes one JSX renders | none | the read is not in the body, and the rendered read tracks |
| a pending async read in a `setTimeout` callback | none | **not claimed**: no `PENDING_ASYNC_UNTRACKED_READ` there; a never-settled source throws `NotReadyError`, which the checker does not report here or in a listener |
| the same pending async read in the body | `SC5001` violation (its `SC1001` is owned by it) | the body read throws `PENDING_ASYNC_UNTRACKED_READ` |
| a read in an `addEventListener` listener, a `PromiseLike.then` callback, a bound argument | `SC1001` violation, **retained** | each can run inside the window; not proven either way |
| a read in a callback of a local function named `setTimeout` | `SC1001` violation | not the standard-library declaration: it runs the callback in the body |
| a read in `setTimeout(wrap(() => ...))` | `SC1001` violation, **retained** | the scheduler receives what `wrap` returns, and the arrow may run on `wrap`'s own stack |

**Stub.** `solid-js.d.ts` is `fresh-stack-scheduler-owner`'s, whose
declarations (`createSignal`, `createMemo` and the rest) are verbatim from the
published rc.3 typings as its header describes. `App.tsx` type-checks cleanly
against that stub and against the real `solid-js`/`@solidjs/signals`/
`@solidjs/web` rc.3 and rc.9 typings (`tsc --noEmit`, TypeScript 5.9.3,
`jsxImportSource: "@solidjs/web"`).
