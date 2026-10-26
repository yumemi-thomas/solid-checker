// every-deck-of-cards ArrangePage.tsx:29 (ArrangePage.test.tsx:25): the component is
// rendered with a spread (`<Walkthrough {...props.rest} />`), so its call sites are
// unknown. An effect's apply hands an element to a method that calls a DOM member
// on it.
import { createEffect, createSignal } from "solid-js";

function createLayoutTransition() {
  function place(surface: HTMLElement, animate: boolean): number {
    const origin = surface.getBoundingClientRect();
    return animate ? origin.left : 0;
  }
  return { place };
}

function Walkthrough(props: { step: number }) {
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

export function Page(props: { rest: { step: number } }) {
  return <Walkthrough {...props.rest} />;
}
