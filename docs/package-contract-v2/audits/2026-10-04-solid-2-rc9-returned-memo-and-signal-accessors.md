# Audit: what `solid-js@2.0.0-rc.9`'s `createMemo` and `createSignal` hand back

Date: 2026-10-04. Supports ADR 0175. Archive: `solid-js@2.0.0-rc.9`,
integrity `sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==`
(the dialect's `AUDITED_ARCHIVES` entry, matched against a registry install's
`bun.lock`), with `@solidjs/signals@2.0.0-rc.9` beside it.

## The question

ADR 0162's computed-accessor row describes a returned accessor by what invoking
it does. The read observes a node the export's own call created. When the node
is stale, the read re-runs the computation that call registered, and it may
throw. Otherwise it invokes nothing. The row was bound to
`@solidjs/signals@2.0.0-rc.9` only. ADR 0162 refused `solid-js`' own factories
"until their returned-value behavior is audited", and the reads audit of
2026-09-30 § 9 refused `solid-js`' `createMemo` because `hydrateSignalLike`'s
*computation* was not walked.

This audit asks a narrower question than that one: **what value does each
build hand back, and what does invoking it do?** The registered computation is
not the read's. It is the creating call's, and its claims are that call's
(`reads`, `creates`, `callbacks`), which for `solid-js` stay refused where the
2026-09-30 audit left them.

## Builds

`exports["."]` selects six files: `browser` and the default select the client
trio, and `node`, `worker` and `deno` select the server trio.

| File | sha256 (prefix) |
| --- | --- |
| `dist/solid.js` | `0238f908 58359bc7` |
| `dist/solid.dev.js` | `7baf8808 f6bd8701` |
| `dist/solid.observe.js` | `c337cd4b 1b90e5dd` |
| `dist/server.js` | `9c25fe06 f9aab765` |
| `dist/server.dev.js` | `129cbe73 5cceed1f` |
| `dist/server.observe.js` | `0d1519f0 613584c5` |

Function bodies below are compared by the sha256 of their exact text (prefix
shown). Each is identical in the three files of its trio unless stated.

## 1. Client builds

Each file imports `createMemo as createMemo$1` and
`createSignal as createSignal$1` from `'@solidjs/signals'` (line 1). The
exports are

    const createMemo = (...args) => (_createMemo || createMemo$1)(...args);
    const createSignal = (...args) => (_createSignal || createSignal$1)(...args);

The only writes to `_createMemo` and `_createSignal` are in `enableHydration()`:
`_createMemo = hydratedCreateMemo; _createSignal = hydratedCreateSignal;`
(`solid.js:704-705`, `solid.dev.js:730-731`, `solid.observe.js:716-717`). So a
call returns either the signals export's own result, or the hydrated wrapper's:

    function hydratedCreateMemo(compute, options) {          // aee74cad1066
      if (!sharedConfig.hydrating || options?.transparent) return createMemo$1(compute, options);
      return hydrateSignalLike(createMemo$1, compute, options);
    }
    function hydratedCreateSignal(fn, second) {               // byte-identical, a185bff0…
      if (typeof fn !== "function" || !sharedConfig.hydrating) return createSignal$1(fn, second);
      return hydrateSignalLike(createSignal$1, fn, second);
    }

`hydrateSignalLike` (`c14803d9c8a2`) has four exits, and each returns
`coreFn(…)`, with `coreFn` the `createMemo$1` or `createSignal$1` passed in:

- `ssrSource === "client"`:
  `withHydrationGate(hydrated => coreFn(prev => …, options))`.
  `withHydrationGate` (`4b5cb7c8577c`) creates a gate signal, calls `create`,
  sets the gate, and returns `create`'s result unchanged.
- `ssrSource === "hybrid"` with a serialized entry: the same
  `withHydrationGate(… coreFn(…))`.
- An async-iterable serialized entry: `hydrateSignalFromAsyncIterable`
  (`41514d7537a6`) returns `null` (falling through) or
  `coreFn(prev => …, options)`.
- Otherwise: `coreFn(prev => readSerializedOrCompute(fn, prev, options), options)`.

So in every client branch the handed-back value is the signals call's own
result. For `createMemo` it is `accessor(computed(…))`. For `createSignal`
slot 0 it is `accessor(node)` on both of its paths: the computed node when the
first argument is a function, the signal node otherwise (reads audit § 5).
Invoking it is the signals `read` the ADR 0162 row was audited on. What the
hydrated branches change is the **registered computation**, now an
archive-authored wrapper around the caller's function that also reads the gate
or the serialized value. That is the creating call's execution. The ADR 0162
premise accounts it there, and `solid-js`' `createMemo` `reads` row stays
refused for exactly that reason.

## 2. Server builds

`createMemo` (`f178813bd95a`, identical in the three) returns either the
`createSyncMemo` result (for `options.sync`) or `read`:

    const read = () => {
      if (!comp.computed) update();
      else if (comp.sync && ctx?.commitEpoch && ctx.commitEpoch() !== comp.epoch) update();
      if (comp.errored) { if (comp.error?.source === CLIENT_HOLE) clientHoleRead(); throw comp.error; }
      return comp.value;
    };

`update` runs `comp.compute`, the registered computation, under the memo's own
owner. `clientHoleRead` throws in every path: in `server.js` (`1fe9e8132aca`)
it throws directly; in `server.dev.js` and `server.observe.js` (`50ce9ddcd34b`)
it first records a dev finding through `recordFinding`, which invokes no
caller code. `createSyncMemo` (`6a648c3d3551`) returns a closure that returns
the cached value, throws the cached error, or calls `pull`, which runs the
registered computation under the memo's owner.

`createSignal` (`38028a6c27de`; `server.dev.js` `6002fc5a126d`, differing only
by `warnServerWrite` calls in the setter):

- a function first argument returns `[createMemo(prev => first(prev), opts), setter]`.
  Slot 0 is the server `read` above; its registered computation is a wrapper
  that calls the caller's function.
- any other first argument returns `[() => first, setter]`. Slot 0 returns
  the captured value and invokes nothing.

## Finding

For `createMemo`'s whole result, and for `createSignal`'s tuple slot 0, in all
six builds, invoking the handed-back accessor does one of three things:

- reads the current value;
- re-runs the computation the creating call registered;
- throws.

It invokes no other callable, and nothing the read was called with matters.
That is the computed-accessor row's description. It is also true of a signal
accessor, which never re-runs anything, so the row may describe a
`createSignal` accessor whose first argument grammar cannot classify.

Not audited here, and unchanged:

- `solid-js`' `createMemo` and `createSignal` `reads` rows. The registered
  computation's own reads under hydration are still unwalked.
- `@solidjs/web`.
- every other `solid-js` release.

The witness still binds the creating call by its resolved declaration and its
archive, and still requires undisplaced, non-spread arguments and
callback-free options (`memo_arguments_discharged`).
