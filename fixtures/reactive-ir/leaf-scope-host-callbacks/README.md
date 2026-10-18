# leaf-scope-host-callbacks

**Claim (ADR 0210).** A standard-library call in a leaf owner's callback runs the functions it is handed on the schedule the audited timing table gives. An inline callback is walked like a helper, and a forbidden operation there is a violation. A fresh-stack or deferred callback runs after the call returns. A `PromiseLike.then` callback, or a callable argument with no audited timing, leaves the obligation open. A setter's updater runs before the setter returns. A member call is read from its resolved declaration, never from the entity at the callee's span, which names the receiver's root (`items().forEach` answers `items`) (`cleanup::member_call_operations`, `cleanup::standard_library_argument_operations`, `cleanup::setter_updater_operations`).

| Case | Finding | Why |
| --- | --- | --- |
| `InlineIdentifier` | `leaf-owner-forbidden-call` violation | `forEach` runs `register` inline |
| `InlineLiteral` | `leaf-owner-forbidden-call` violation | the same through a literal |
| `InlineInHelper` | `leaf-owner-forbidden-call` violation | the same one helper down |
| `InlineClean` | none | the inline callback only logs |
| `FreshStack` | none | `setTimeout` runs its callback from a host queue |
| `Listener` | none | a listener runs after the call returns; synchronous dispatch is not modeled |
| `Thenable` | `SC9012` uncertifiable | a `PromiseLike`'s `then` is not the host's and may call back before it returns |
| `UnauditedCallback` | `leaf-owner-forbidden-call` violation | the `Promise` executor runs inline (ECMA-262 `Promise(executor)` step 10, ADR 0217) and calls `register` |
| `SetterUpdater` | `leaf-owner-forbidden-call` violation | the updater registers a cleanup |
| `SetterUpdaterClean` | none | the updater only computes |
| `BindDoesNotRun` | none | `bind` runs nothing; the walk used to read `register.bind` as a call of `register` |
| `CallRunsReceiver` | `leaf-owner-forbidden-call` violation | `call` runs `register` |

`solid-js.d.ts` is copied from `leaf-scope-exact-method`, except that the setter takes an updater, as `@solidjs/signals`' `Setter<T>` does.

`AugmentedTimers.tsx` and the project-owned `timers.d.ts` exercise an ambient
overload selected outside the default library for the same global binding.
The checker retains the selected signature and independently joins the exact
global symbol to its default-library declaration. Only already-audited global
schedulers consume that identity. Native timer callbacks run on fresh stacks;
a declared lookalike remains uncertifiable, and the source-visible shadow
registers cleanup in the leaf scope. These declarations are not a package stub
or a package behavior model. The source passes strict and loose checks against
published Solid RC.13 typings without this fixture's declaration stubs.

The retained-session twin adds and removes a timer assignment in another file,
then repeats through `window`. The original caller is unchanged; its leaf
obligation must appear and disappear, with retained and cold IR equal. Direct
aliases, imports and structural members do not acquire the new binding fact.

`Focus.tsx` pins the unresolved DOM focus boundary. Since ADR 0318 the
optional form is decided per member: an optional call of an audited
script-free `lib.dom` member (`el?.querySelector(…)`, see
`leaf-scope-optional-dom`) is certified like its non-optional form, while a
re-entrant member such as `focus` stays open in the optional form. The
non-optional `el.focus()` is still certified by the standard-library trust,
which does not model dispatch; ADR 0318 records that inconsistency. An optional
DOM `focus` and a caller-supplied `focus` implementation remain SC9012 uncertifiable,
including when a focus listener visibly registers cleanup. The listener's
registration does not prove whether it will run at this focus call. Exact local
instances are controls: an inert `focus` method is clean and a method that calls
`onCleanup` produces SC3001. No trust follows from the method's spelling or its
DOM type. The new file also passes `tsc --noEmit` against published Solid
2.0.0-rc.13 typings without this fixture's stub.

The [HTML focus update steps](https://html.spec.whatwg.org/multipage/interaction.html#focus-update-steps)
fire blur/focus events during focusing. Discharging a leaf obligation therefore
requires exact runtime dispatch and evidence about synchronously invoked
listeners, including listeners on the previously focused element and ancestors.
The existing `Listener` case covers registration alone, not event dispatch.

`Fetch.tsx` consumes the same exact merged-global binding for native `fetch`.
The overload in `timers.d.ts` preserves the default library's signature
byte-faithfully. Selected parameter mappings remain authoritative; this adds
no callback timing claim. Native and transparently wrapped fetch calls are
clean, including the fetch step of an async helper whose remaining calls run
after an unconditional await. A declared lookalike and literal getter options
remain SC9012; a source-visible shadow registering cleanup produces SC3001
alongside its existing helper ownership obligation (SC4001).
The cases pass strict and loose TypeScript checks against published Solid
2.0.0-rc.13 typings without the fixture's Solid stub.
