import { onCleanup, onSettled } from "solid-js";

// An object literal key with an escape handed to `Object.assign`: the cooked key is `onfocus`.
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
    Object.assign(el, { "\u006fnfocus": h });
  }} />;
}
