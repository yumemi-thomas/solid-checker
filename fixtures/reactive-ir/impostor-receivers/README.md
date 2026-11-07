# impostor-receivers

ADR 0874: a receiver ADR 0840 exempted is exempt only when no code of the program can reach it.

- `HandedOn`, `ReduceHandedOn`, `ThisHandedOn`, `NamedCallback`: a callback or a prototype method gets the array; its `forEach` stays
  uncertifiable (`standard-member-impostor`), never a violation. Chrome: the impostor defers, the write runs outside the memo, quiet.
- `CleanArity`: a literal callback that stops short of the receiver's slot leaves the exemption: the write is a violation.
- `HostWritten`: a member written on `document`, on an alias or through a helper withdraws every host receiver of that member.
- `Species`: `(Array.prototype as any).constructor` withdraws the chained receiver.
- `Replace`: `replace` with an object first argument dispatches through it; with a string it stays standard.
- `Thenable`: a `then` on `Object.prototype` makes the awaited array thenable.
