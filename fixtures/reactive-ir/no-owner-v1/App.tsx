// v1/missing-owner   (SC4001): the 1.x exit of the owner-presence rule. The
// unprefixed 2.0 twin is pinned by fixtures/reactive-ir/owner-presence; this
// project resolves solid-js 1.9.14, so the v1 catalog runs and the finding
// must come out under the v1/ name. Note the 1.x signature: the callback is
// argument 0 and argument 1 is a seed value, so a one-argument createEffect
// is complete here (in 2.0 the same call would also be SC7001).
import { createEffect, createRoot, createSignal, Suspense } from "solid-js";
import { readCount } from "uncontracted-package";

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

// v1/missing-owner     (SC4001): Suspense creates an ownership boundary in
// Solid 1.x. At module scope there is no parent owner to dispose it, so the
// boundary and the work retained below it leak for the lifetime of the app.
export const OrphanBoundary = (
  <Suspense fallback={<div />}>
    <div>{ticks()}</div>
  </Suspense>
);

// A capitalized exported function is only conventionally a component. Without
// a JSX callsite or Component annotation it could instead be called as an
// ordinary helper, so its owner status is explicitly uncertifiable.
export function App() {
  createEffect(() => {
    console.log(ticks());
  });

  // Solid 1.x ref callbacks preserve the component owner through untrack(), so
  // creating state or computations here participates in normal disposal and
  // does not emit the 2.0-only directive-application rule as expected.
  // readCount() also makes the uncontracted Solid-aware package observable,
  // pins v1/package-contract-incomplete (SC9005) at its import above.
  return <div ref={element => createSignal(element)}>{readCount()}</div>;
}
