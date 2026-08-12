// v1/no-owner-effect (SC4001): the 1.x exit of the owner-presence rule. The
// unprefixed 2.0 twin is pinned by fixtures/reactive-ir/owner-presence; this
// project resolves solid-js 1.9.14, so the v1 catalog runs and the finding
// must come out under the v1/ name. Note the 1.x signature: the callback is
// argument 0 and argument 1 is a seed value, so a one-argument createEffect
// is complete here (in 2.0 the same call would also be SC7001).
import { createEffect, createRoot, createSignal } from "solid-js";

const [ticks] = createSignal(0);

// Module scope has no reactive owner: nothing will ever dispose this effect,
// so it keeps its subscription to ticks for the lifetime of the app.
createEffect(() => {
  console.log(ticks());
});

// The corrected form wraps the setup in createRoot, which supplies the owner
// and the dispose handle, and must stay silent.
createRoot(dispose => {
  createEffect(() => {
    console.log(ticks());
  });
  return dispose;
});

// Inside a component the render root owns the effect: also silent.
export function App() {
  createEffect(() => {
    console.log(ticks());
  });
  return <div />;
}
