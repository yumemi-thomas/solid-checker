# The built-in runtime-semantics matrix

One entrypoint exercising the whole table of standard-library callees whose
callback timing the analyzer knows without a package contract. The three
answers it has to keep apart:

- **`same-stack`**: `Array.from` / every `TypedArray.from` mapper (argument 1),
  and the `String.prototype.replace` / `replaceAll` replacer. These run before
  the call returns.
- **`queued`**: the observer constructors (`ReportingObserver`,
  `IntersectionObserver`), `scheduler.postTask`, both geolocation callbacks
  (`getCurrentPosition`, `watchPosition` -- two separate parameters), and the
  container retentions (`Array#push`, `Set#add`, `Map#set`), where the value is
  stored and may be reached later.
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
