import { onCleanup, onSettled } from "solid-js";

// `focusin` bubbles: the runtime delegates it to the document, and the handler runs on the stack of `focus()`.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} onFocusIn={() => onCleanup(() => {})} />;
}
