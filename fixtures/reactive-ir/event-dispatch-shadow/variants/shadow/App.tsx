import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// ADR 1115: the same delivery to a listener in a shadow tree runs with `window.event` unset, so the apply's strict-read window holds
// for the read (Chrome rc.21: STRICT_READ_UNTRACKED). The read stays uncertifiable.
export function Shadow() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  document.createElement("div").attachShadow({ mode: "open" }).appendChild(box);
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
    },
  );
  return <div />;
}
