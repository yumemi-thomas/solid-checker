import { onCleanup, onSettled } from "solid-js";

// The IDL handler property.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    el.onfocus = () => {
      onCleanup(() => {});
    };
  }} />;
}
