import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1125 (G3): the shadow root attached under a concatenated key, which ADR 1115's text test does not read; the facts decode it.
export function ShadowConcat() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  (document.createElement("div") as unknown as Record<string, (init: ShadowRootInit) => ShadowRoot>)["attach" + "Shadow"]({ mode: "open" }).appendChild(box);
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}
