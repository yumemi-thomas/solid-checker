# forwarded-event-prop

**Claim (ADR 0199).** A read in a function literal written as a project component's prop runs on DOM event dispatch, never in the caller's strict-read window, when the component only hands that prop to an intrinsic element's event handler (directly, in a wrapping handler, via `merge`/`omit` views, spreads, or another component's prop meeting the same proof): `ExecutionRole::EventCallback`, not reported; render-time calls violate; spreads to a caller, eager forwards, `merge` escapes, mixed uses, cycles are uncertifiable. `merge`/`omit` copy `@solidjs/signals` rc.13 `store/utils.d.ts`.

Variant `eval-handler`: `EvalHandler.tsx`, moved here from this project. ADR 0803 (item 3): a direct `eval` anywhere in a project may patch array iteration, which withdraws every primitive's tuple slots in it; the eval cases live in `variants/eval-handler` so the cases here keep their proofs, and the variant pins the withdrawal.
