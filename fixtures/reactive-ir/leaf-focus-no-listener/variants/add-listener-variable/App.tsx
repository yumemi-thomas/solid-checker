import { onCleanup, onSettled } from "solid-js";

// The type is built: no fact says it is not `focus`.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    const type: string = "foc" + "us";
    el.addEventListener(type, () => onCleanup(() => {}));
  }} />;
}
