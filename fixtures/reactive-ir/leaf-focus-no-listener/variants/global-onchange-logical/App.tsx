import { onCleanup, onSettled } from "solid-js";

// A logical assignment to the global handler property: `onchange ??= h`.
const h = () => {
  onCleanup(() => {});
};
onchange ??= h;

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
