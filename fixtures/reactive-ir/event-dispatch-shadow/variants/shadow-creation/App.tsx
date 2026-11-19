import { createEffect, createRenderEffect } from "solid-js";

// ADR 1115 part 2: a creation in the handler of a listener in a shadow tree, delivered by `dispatchEvent` from an effect's apply. The
// listener runs with `window.event` unset, so the handler is inside the apply's effect-callback window and the creation throws
// PRIMITIVE_IN_EFFECT_CALLBACK first (Chrome rc.21, mx-census `evt-apply-dispatch-shadow`). No missing-owner violation stands there.
export function ShadowCreation() {
  const box = (<div onScroll={() => { createRenderEffect(() => 1, () => {}); }} />) as HTMLDivElement;
  document.createElement("div").attachShadow({ mode: "open" }).appendChild(box);
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}
