// app-game abr-viewer App.tsx:39 (routes.tsx:9, escaped-lazy-routes.tsx): the
// component is reached through a lazy route's dynamic import (`module.Page`), so
// its call sites are unknown. An effect's apply hands an element to a method that
// calls a DOM member on it.
import { createEffect, createSignal } from "solid-js";

function createLayoutTransition() {
  function place(surface: HTMLElement, animate: boolean): number {
    const origin = surface.getBoundingClientRect();
    return animate ? origin.left : 0;
  }
  return { place };
}

export function Page(props: { step: number }) {
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
