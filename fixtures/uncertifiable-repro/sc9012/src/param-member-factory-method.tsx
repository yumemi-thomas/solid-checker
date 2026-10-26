// every-deck-of-cards AlgorithmWalkthrough.tsx:32 (presentation/layout-transition.ts:5):
// an effect's apply calls a method of a factory's result (a shorthand `{ place }`)
// that calls a DOM member on the element it is handed.
import { createEffect, createSignal } from "solid-js";

function createLayoutTransition() {
  function place(surface: HTMLElement, animate: boolean): number {
    const origin = surface.getBoundingClientRect();
    return animate ? origin.left : 0;
  }
  return { place };
}

export function Walkthrough(props: { step: number }) {
  const motion = createLayoutTransition();
  const [surface, setSurface] = createSignal<HTMLElement>();
  createEffect(
    () => ({ el: surface(), step: props.step }),
    ({ el, step }) => {
      if (el) motion.place(el, step > 0);
    },
  );
  return <div ref={setSurface} />;
}
