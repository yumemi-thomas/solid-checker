import { onCleanup, onSettled } from "solid-js";

// The global object's own handler property: `onfocus = h` (no receiver) is `window.onfocus = h`.
const h = () => {
  onCleanup(() => {});
};
onfocus = h;

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
