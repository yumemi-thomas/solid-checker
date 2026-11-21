import { onCleanup, onSettled } from "solid-js";

// A handler for `focus` on the element: it runs on the stack of the leaf callback, where `onCleanup` is refused.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} onFocus={() => onCleanup(() => {})} />;
}
