import { onCleanup, onSettled } from "solid-js";

// The bare global `addEventListener` (the window's) written with an escape: the cooked name is the global registrar.
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
    \u0061ddEventListener("focus", h);
  }} />;
}
