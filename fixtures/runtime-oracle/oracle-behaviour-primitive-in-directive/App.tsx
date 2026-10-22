// SC6001 primitive-in-directive-application, observed behaviourally. An effect
// created in a directive's apply callback runs with no owner and is never
// disposed, so it keeps running after its element is removed. The control creates
// its effect in the factory's setup phase, owned by the element's scope, so hiding
// the element disposes it.
import { createEffect, createSignal, Show } from "solid-js";

// The probe counts effect runs on window; the observations read it there.
const probe = { violationRuns: 0, controlRuns: 0 };
Object.assign(globalThis, { oracleProbe: probe });

const leakingTrack = (count: () => number) => (element: HTMLElement) => {
  createEffect(() => count(), () => { probe.violationRuns += 1; });
  void element;
};

const ownedTrack = (count: () => number) => {
  createEffect(() => count(), () => { probe.controlRuns += 1; });
  return (element: HTMLElement) => { void element; };
};

export function Violation() {
  const [count, setCount] = createSignal(0);
  const [shown, setShown] = createSignal(true);
  return (
    <section>
      <button data-probe="violation-hide" onClick={() => setShown(false)}>hide</button>
      <button data-probe="violation-write" onClick={() => setCount(1)}>write</button>
      <Show when={shown()}>
        <div data-probe="violation-tracked" ref={leakingTrack(count)} />
      </Show>
    </section>
  );
}

export function Control() {
  const [count, setCount] = createSignal(0);
  const [shown, setShown] = createSignal(true);
  return (
    <section>
      <button data-probe="control-hide" onClick={() => setShown(false)}>hide</button>
      <button data-probe="control-write" onClick={() => setCount(1)}>write</button>
      <Show when={shown()}>
        <div data-probe="control-tracked" ref={ownedTrack(count)} />
      </Show>
    </section>
  );
}
