import { createSignal, createEffect, onCleanup } from "solid-js";

import { handlers } from "./lib";

export function Flow() {
  const [count] = createSignal(0);
  const h = handlers(() => {
    createEffect(() => count(), () => undefined);
    onCleanup(() => undefined);
  });
  return <button {...h}>b</button>;
}

export function FlowProp() {
  const [count] = createSignal(0);
  const h = handlers(() => {
    onCleanup(() => undefined);
  });
  return <button onClick={h.onClick}>c</button>;
}
