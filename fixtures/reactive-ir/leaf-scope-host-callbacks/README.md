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
| `UnauditedCallback` | `SC9012` uncertifiable | no audited timing for the `Promise` executor |
| `SetterUpdater` | `leaf-owner-forbidden-call` violation | the updater registers a cleanup |
| `SetterUpdaterClean` | none | the updater only computes |
| `BindDoesNotRun` | none | `bind` runs nothing; the walk used to read `register.bind` as a call of `register` |
| `CallRunsReceiver` | `leaf-owner-forbidden-call` violation | `call` runs `register` |

`solid-js.d.ts` is copied from `leaf-scope-exact-method`, except that the setter takes an updater, as `@solidjs/signals`' `Setter<T>` does.
