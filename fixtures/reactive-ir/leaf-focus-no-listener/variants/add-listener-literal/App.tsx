import { onCleanup, onSettled } from "solid-js";

// A literal `focus` listener, registered where the element is made.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    el.addEventListener("focus", () => onCleanup(() => {}));
  }} />;
}
