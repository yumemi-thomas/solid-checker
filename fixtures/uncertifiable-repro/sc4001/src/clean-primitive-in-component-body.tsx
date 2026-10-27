// Clean. Mirrors every-deck-of-cards src/Layout.tsx:10-12: an effect and a cleanup created in the
// component body have an owner; in a ref callback they have none (primitive-in-ref-callback).
import { createEffect, createSignal, onCleanup } from "solid-js";

export function Layout() {
  const [wide] = createSignal(true);
  createEffect(wide, (enabled) => {
    document.body.classList.toggle("viewport-layout", enabled);
  });
  onCleanup(() => document.body.classList.remove("viewport-layout"));
  return <main />;
}
