// solid-js and @solidjs/web rc.17 over @solidjs/signals rc.13: no review read
// this combination, and `dispatchAsInteraction`'s dev build calls a signals
// `withInteraction` rc.13 was not read for, so neither rc.17 export is
// modelled. The same calls `rc17-dispatch-as-interaction` and
// `rc17-dynamic-component` prove there state nothing here.
import { createEffect, createSignal, dynamicComponent } from "solid-js";
import { dispatchAsInteraction } from "@solidjs/web";

const [count, setCount] = createSignal(0);
const event = new Event("probe");
const Plain = () => null;

// Not claimed: a write inside an unmodelled callback.
export function WriteInBody() {
  dispatchAsInteraction(event, () => setCount(1));
  return null;
}

// Not claimed: an effect inside an unmodelled static source.
export const StaticEffect = dynamicComponent(() => {
  createEffect(() => count(), () => {});
  return Plain;
}, { static: true });
