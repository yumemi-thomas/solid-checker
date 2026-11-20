import { onCleanup, onSettled } from "solid-js";

// The standard member is replaced where the program runs.
HTMLElement.prototype.focus = function () {
  onCleanup(() => {});
};

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
