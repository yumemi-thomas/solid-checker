import { onCleanup, onSettled } from "solid-js";

// The standard member is replaced by a definer.
Object.defineProperty(HTMLElement.prototype, "focus", {
  value() {
    onCleanup(() => {});
  },
  configurable: true,
});

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
