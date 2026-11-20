import { onCleanup, onSettled } from "solid-js";

// The method is bound and called under another name.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    const add = el.addEventListener.bind(el);
    add("focus", () => onCleanup(() => {}));
  }} />;
}
