# Package parameter-member consumer

The reviewed contract says `drop` reads its parameter 0 as a
`parameter-member`. Whether that read is reactive therefore depends on what
each call site hands it, and the consumer must prove the negative rather than
assume it.

- `ReactiveArgument` passes the contract-returned store, so the member read is
  a proven store-path read outside tracking (`SC1001`).
- `PlainArgument` passes a spread-free array literal — a value created at the
  call site, which `createState` cannot have produced — so it is proven plain
  and stays clean.
- `UnknownArgument` passes a bare `declare const`, whose origin the project
  cannot see; the `SC9012` obligation is kept.
- `SpreadArgument` passes `[...state]`. Spreading copies out of the proxy at
  the call site, so the callee really does receive snapshot data and its
  parameter-member read proves nothing about reactivity. The read that exists
  is the spread, and the spread pass reports it (`SC1001`) in its own
  execution role — exactly once, at the spread. No `SC9012` is added here, and
  adding one would report a single dependency twice.
- `SpreadOfLiteralArgument` is the negative for that: the same shape built
  from plain data reads nothing and stays clean.

What the spread pair does *not* cover is a nested proxy surviving the shallow
copy (`drop({ ...store }).nested.value`); that residual is recorded in
docs/precision-backlog.md.

The declarations are exact for this fixture package; every finding depends on
the runtime contract, not on trusting the declaration as runtime evidence.
