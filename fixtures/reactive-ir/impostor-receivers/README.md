# impostor-receivers

ADR 0874: a receiver ADR 0840 exempted is exempt only when no code of the program can reach it.

- `HandedOn`, `ReduceHandedOn`, `ThisHandedOn`, `NamedCallback`: a callback or a prototype method gets the array; its `forEach` stays
  uncertifiable (`standard-member-impostor`), never a violation. Chrome: the impostor defers, the write runs outside the memo, quiet.
- `HostWritten`: a member written on `document`, on an alias or through a helper withdraws every host receiver of that member.
- `Species`: `(Array.prototype as any).constructor` withdraws the chained receiver.
- `Replace`: `replace` with an object first argument dispatches through it, so a read of `Symbol.replace` opens every `replace` in the program: the string and regex calls here too. Their proof is `impostor-host-intact`.
- `Thenable`: a `then` on `Object.prototype` makes the awaited array thenable.
