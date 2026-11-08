# SC5001: key probes of an uninitialized derive

`Object.keys`, `in`, `hasOwn` and `for…in` read no key on a settled store (ADR 0621). An
uninitialized derive holds every trap: they throw `PENDING_ASYNC_UNTRACKED_READ` in a body,
through `untrack` too (ADR 0635). Chrome, rc.21.
