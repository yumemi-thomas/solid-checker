# The built-in runtime-semantics matrix

One entrypoint exercising the whole table of standard-library callees whose
callback timing the analyzer knows without a package contract. The three
answers it has to keep apart:

- **`same-stack`**: `Array.from` / every `TypedArray.from` mapper (argument 1),
  and the `String.prototype.replace` / `replaceAll` replacer. These run before
  the call returns.
- **`queued`**: the observer constructors (`ReportingObserver`,
  `IntersectionObserver`), `scheduler.postTask`, both geolocation callbacks
  (`getCurrentPosition`, `watchPosition` -- two separate parameters).
- **retained, invocation unknown**: `Array#push`, `Set#add` and `Map#set`
  store the value without proving it will ever be called (ADR 0023).
- **no claim at all**: the conversion functions (`Number`, `Boolean`, `BigInt`,
  `Symbol`, `Object`), `new Array`, and the collection constructors, which
  never invoke an argument.

`shadowedString` and `shadowedQueueMicrotask` are the negative controls: a
module-local function of the same name is what the call resolves to, so the
built-in entry must not apply. `runtime-semantics-shadowed` pins the same
control for a *constructor*.

Nothing here is `closed`: these are descriptions of possible operations, not
proofs, so an entry that disappears shows up as a vanished operation rather
than as a refusal.

## Retention correction (ADR 0023)

`retainArray`, `retainMap` and `retainSet` no longer propose queued callback
invocations: storage alone proves none. Their callback domains remain unknown;
the existing direct and scheduler cases preserve their invocation proposals.
The reviewed main and proposal snapshots remove only those storage claims.
