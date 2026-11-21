import { onCleanup, onSettled } from "solid-js";

// The program itself registers nothing; the page decides.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
