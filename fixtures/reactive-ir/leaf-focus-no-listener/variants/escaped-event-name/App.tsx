import { onCleanup, onSettled } from "solid-js";

// The event type is a string with an escape: its cooked value is `focus`, a listener for what a focus move delivers.
const h = () => {
  onCleanup(() => {});
};
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    el.addEventListener("\u0066ocus", h);
  }} />;
}
