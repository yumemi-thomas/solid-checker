import { onCleanup, onSettled } from "solid-js";

// A handler key assigned onto the element.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    Object.assign(el, { onfocus: () => onCleanup(() => {}) });
  }} />;
}
