import { onCleanup, onSettled } from "solid-js";

// The handler property under brackets with an escape: the cooked key is `onfocus`.
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
    el["\u006fnfocus"] = h;
  }} />;
}
