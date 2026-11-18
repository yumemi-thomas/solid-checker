# store-setter-after-await (ADR 1089 part D)

rc.21's plain store setter (`createStore(value)`, `@solidjs/signals` 2.0.0-rc.21 `dist/dev.js:6819-6920, 9742-9744`) calls the
one function it is given with the store's proxy on the caller's stack, then notifies through the engine's own hooks. So after an
await, a call of it runs no project code but its updater. An updater whose whole body writes literals under static keys of its
draft or of the store root reads nothing (`Dialect::store_setter_calls_updater`, `library_call::updater_writes_literals_only`).

- `ThenDraft`, `AwaitRoot`: no SC1002 row at the setter call.
- `ReadsSignal` (a signal read), `ReadsMember` (a member read), `Derived` (`createStore(fn, seed)`): the row stays.
