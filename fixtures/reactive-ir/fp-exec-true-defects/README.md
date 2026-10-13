# fp-exec-true-defects

**Claim.** Inverting the lexical default (code in a component is body-time only
when proven) must not remove the defects the Solid dev runtime warns about for
real. These two came out of the 2026-10 development-feedback sweep and are
kept as positive controls for `SC1001`:

| Case | Finding | Why |
| --- | --- | --- |
| `Button`: `const variant = props.variant ?? "primary"`; `Filters` passes `variant={filter() === "all" ? ...}` | `SC1001` violation | a prop read once in the component body; the parent drives it from a signal, so the buttons never restyle (queue-management-ui `button.tsx:36`) |
| `StopRow`: `onClick={props.canAlight ? props.onAlight : props.onBoard}`; `Route` passes `canAlight={boardSeq() !== null && ...}` in a `<For>` row | `SC1001` violation | an event-handler attribute is evaluated once at creation (`_$addEvent`), so a later `canAlight` change never swaps the handler (probus-hk `RouteDetail.tsx:1555`) |

The body-time controls (direct body read, immediately invoked function,
`[1].map(() => n())`, a default parameter called in the body, `<Show>`/`<For>`
callback reads) live in `fp-exec-stored-literals` and
`fp-exec-control-flow-callback`.

**Stub.** `solid-js.d.ts` is the control-flow fixture's (`createSignal`, `For`,
`Show` and their helper types from the published rc.9 typings) with one
intrinsic element added. `App.tsx` passes `tsc --noEmit` (TypeScript 5.9.3,
`strict`) against the stub and against the real rc.9 install with
`jsxImportSource: "@solidjs/web"`.
