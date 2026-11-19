# event-dispatch-shadow (ADR 1115)

rc.21 opens an effect apply's strict-read window keyed on the event current when its queue started (`@solidjs/signals`
`dist/dev.js:2771`, `dev-shared.js:5796, 5803`). A listener the apply delivers synchronously in the light tree runs with
`window.event` set, outside the window (ADR 0639). A listener in a shadow tree runs with `window.event` unset, inside it: an
untracked read there warns `STRICT_READ_UNTRACKED` (Chrome rc.21). The read proof of a delivery from an apply or a handler
holds only while no listener of the program may sit in a shadow tree (`event_delivery::scan_shadow_roots`).

- `Light` (this project): the read is proved.
- `variants/shadow`: the element sits in a shadow root: the read stays uncertifiable.
- `variants/custom-element`: the light program and a `customElements.define`: the program may hold a shadow tree, so the read
  stays uncertifiable.
