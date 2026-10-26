// kui kernel RecordRow.tsx:108 (RecordRow.stories.tsx:26, `component: RecordRow`):
// the component is listed in an object (`{ component: Walkthrough }`), so its call
// sites are unknown. An effect's apply hands an element to a method that calls a
// DOM member on it.
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

export const routes = [{ path: "/walk", component: Walkthrough }];
