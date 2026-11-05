# SC1001: a Get of a value escaped from `untrack`

`const o = untrack(() => s.o); o.x` warns on rc.21: `untrack` scopes the wrapped read,
and a Get of the proxy it returns is a read of its own (rules-mining CS-R19, ADR 0629).
The store's literal initial value proves the proxy; a primitive or a `snapshot` is no
proxy; a prop, a `Date` slot, a helper's result or a hand-off stays open. Tracked and
event code reads it freely. Measured on rc.21 in Chrome.
