# ADR 0210: Host callbacks and member calls in a leaf scope

- Status: accepted and implemented (2026-10-06). Step 3 of the 2026-10-06
  plan (calls in leaf-owner callbacks).
- Owners:
  - `member_call_operations`, `standard_library_argument_operations`,
    `setter_updater_operations` and `argument_body_operations`
    (`solid-reactive-ir/src/cleanup.rs`);
  - the member spellings of the host schedulers in `timing_behavior` and
    `FRESH_STACK_SCHEDULERS` (`solid-reactive-ir/src/runtime_semantics.rs`).
- Relation: corrects the leaf-scope helper walk (ADR 0179, ADR 0192, ADR
  0209).

## Context

A leaf owner's callback (`onSettled`, `createTrackedEffect`) must create no
primitive, register no cleanup and flush nothing. The walk follows each call
in the callback. It had three faults, two of them wrong clean results:

1. **A standard-library call was clean, whatever it was handed.** The host
   runs no Solid code of its own, but it runs the functions it is given.
   `items.forEach(register)` runs `register` before it returns, so an
   `onCleanup` in `register` runs in the leaf scope. The walk certified the
   callback clean. So did `forEach(() => onCleanup(…))`, a `new Promise`
   executor and a setter's updater.
2. **A member callee was read from the entity at its complete span.** For a
   member expression the compiler answers there with the receiver's root
   binding: `list.forEach` answers `list`, `items().forEach` answers `items`
   and `register.bind` answers `register`.
   - `items().forEach(register)` was therefore an accessor call, which is
     safe, and certified clean.
   - `register.bind(null)` was a call of `register`. A forbidden operation in
     `register` was reported although `bind` runs nothing. That was a false
     positive.
3. `window.setTimeout(…)` resolves to the `WindowOrWorkerGlobalScope`
   member, which the timing table did not list, unlike the global function.

## Decision

1. **A standard-library call's arguments are classified by the audited
   timing table** (`runtime_semantics::argument_behavior`):
   - an inline callback is walked like a helper's body. A forbidden
     operation there is a violation at this call.
   - a fresh-stack callback (`setTimeout`, `queueMicrotask`,
     `requestAnimationFrame`) runs from a host queue, after the leaf scope
     is gone;
   - a deferred callback (every default-library `addEventListener`
     listener, `bind`'s bound arguments) runs after the call returns;
   - a `PromiseLike.then` callback may run before the call returns. The
     thenable is any object with a `then`, and its implementation is not the
     host's. A forbidden operation there, or a body the walk cannot follow,
     leaves the `SC9012` obligation open. It is never a violation;
   - a value the host only reads or keeps is not run;
   - a callable parameter with no audited timing (`new Promise(executor)`,
     `Array.from(x, map)`) leaves the obligation open when it is handed
     anything that may be a function.
   - A parameter typed `any` or `unknown` (`console.log(...data)`) is read as
     a value.
2. **Only an exactly known function is walked.** That is a function literal
   written as the argument, or an identifier bound to a project function.
   Anything else may be called, so it leaves the obligation open.
3. **A setter's function argument is its updater**, which the setter runs
   before it returns. It is walked as an inline callback.
4. **A member call is read from its resolved declaration, never from the
   entity at the callee's span.**
   - An exact instance's method is followed as before (ADR 0209).
   - A standard-library member is followed by 1, and when it is `call` or
     `apply`, its receiver is walked as an inline callback.
   - Any other member call leaves the obligation open.
5. **The member spellings of the host schedulers are the same functions.**
   - `WindowOrWorkerGlobalScope.setTimeout`, `.setInterval` and
     `.queueMicrotask`, `AnimationFrameProvider.requestAnimationFrame` and
     `Window.requestIdleCallback` join the timing table.
   - They also join the fresh-stack list beside their global spellings.

## Consequences

- New violations: a forbidden operation reached through `forEach`, `map`,
  `call` and the other inline callbacks, and through a setter's updater.
- New obligations: `PromiseLike.then` callbacks and unaudited callable
  arguments whose bodies are not proven clean.
- Removed false positives: `fn.bind(…)` and other non-invoking members of a
  project function.
- Still not modeled:
  - synchronous dispatch. `el.click()` or `el.focus()` in a leaf scope runs
    listeners there, including ones registered by this callback. The walk
    has always read such a call as a host call that runs only what it is
    handed, and a listener registered in a leaf scope is read the same way.
    A first measurement that kept listeners open added 169 obligations on
    the rc.13 corpus, most of them `addEventListener` calls in `onSettled`;
  - implicit invocation through getters, `toString`, `valueOf` or an
    iterator. The rest of the standard-library trust does not model it
    either.
- A member call on a namespace import (`utils.helper()`) still leaves the
  obligation open, as it did before.

## Evidence

- **Fixture** `fixtures/reactive-ir/leaf-scope-host-callbacks`:
  - `SC3001` violations for `forEach` with an identifier, with a literal and
    one helper down, for a setter's updater and for `register.call(null)`;
  - `SC9012` for a `PromiseLike` thenable and for a `Promise` executor;
  - clean for an inline `console.log`, a `setTimeout` callback, a listener,
    a pure updater and `register.bind(null)`.
- **Coverage:** 179 fixture projects, 946 findings. Only the new snapshot
  is new. `fp-exec-handler-writes` moved while the member scheduler
  spellings were missing and is unchanged with them.
- **rc.13 corpus**, browser host, release binary, against
  `rc13-l-browser.json`:
  - violations unchanged at 285;
  - uncertifiable 3,380 to 3,414: 52 added, 16 removed.
- **The 52 added** are wrong clean results made honest, or calls nobody has
  audited:
  - member calls on a call's result, which certified clean by walking the
    root function (`appPort().servers.onInvite(cb)`,
    `canvas.observeViewport(el, cb)`);
  - setter updaters inside helpers whose bodies the walk cannot follow;
  - host APIs with no audited timing: `navigator.mediaSession
    .setActionHandler` (7), `MediaQueryList.addListener` and the global
    `addEventListener`;
  - `then` chains on values that are not proven `Promise`s.
- **The 16 removed** are standard-library members on a call's result
  (`getSetting(key).trim()`), which had no entity at the callee before.
