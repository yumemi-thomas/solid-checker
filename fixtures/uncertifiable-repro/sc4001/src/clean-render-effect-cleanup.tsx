// Clean. Mirrors skyjtx-website apps/skyjt-website/src/presentations/about/useParallax.ts:116-150: a
// render effect computes an element and its apply registers a listener, returning the teardown.
import { createRenderEffect, createSignal } from "solid-js";

export function Parallax() {
  const [target, setTarget] = createSignal<HTMLDivElement>();
  createRenderEffect(
    () => target(),
    (element) => {
      if (!element) return;
      const move = () => element.setAttribute("data-moved", "true");
      element.addEventListener("pointermove", move);
      return () => element.removeEventListener("pointermove", move);
    },
  );
  return <div ref={setTarget} data-probe="parallax">parallax</div>;
}
