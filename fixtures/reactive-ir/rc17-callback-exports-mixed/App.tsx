// solid-js and @solidjs/web rc.17 over @solidjs/signals rc.13: no review read
// this combination, and `dispatchAsInteraction`'s dev build calls a signals
// `withInteraction` (and `solid-js`'s client `createDeferred` a signals
// `createDeferred`) rc.13 was not read for, so no rc.17 callback export is
// modelled. The same calls the `rc17-*` fixtures prove there state nothing
// here.
import {
  createDeferred,
  createEffect,
  createSignal,
  dynamicComponent,
  lazyModule,
  onHydrationEnd,
} from "solid-js";
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

// Not claimed: a write inside an unmodelled `createDeferred` compute.
export function DeferredWrite() {
  createDeferred(() => setCount(2));
  return null;
}

// Not claimed: an effect `lazyModule`'s loader creates when invoked at module
// scope, and one in an `onHydrationEnd` callback.
const load = lazyModule(() => {
  createEffect(() => count(), () => {});
  return Promise.resolve({});
});
load();
onHydrationEnd(() => createEffect(() => count(), () => {}));
