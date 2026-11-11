# impostor-receivers

ADR 0874: a receiver ADR 0840 exempted stays exempt only while no program code can reach it.
- `HandedOn`, `ReduceHandedOn`, `ThisHandedOn`, `NamedCallback`: code gets the array; its `forEach` is uncertifiable (Chrome: quiet).
- `HostWritten`: a member written on `document` (an alias, a helper) withdraws every host receiver of it.
- `Species`: `(Array.prototype as any).constructor` withdraws the chained receiver.
- `Replace`: a read of `Symbol.replace` opens every `replace` (proof: `impostor-host-intact`).
- `Thenable`: `then` on `Object.prototype` makes the awaited array thenable.

ADR 0951: the program's one reflective definer is `Object.defineProperty(Object.prototype, "then", …)` (`Thenable`), a literal key. It withholds calls of `then`; it no longer withholds the calls of members no write names (`queueMicrotask`, the project class's `forEach` method, `Symbol.replace` …): 8 rows fewer. The `forEach` and `addEventListener` rows stand on the open writes of those keys and on the impostor census.
