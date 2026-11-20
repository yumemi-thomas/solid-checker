import { onCleanup, onSettled } from "solid-js";

// The handler property written with an escaped identifier: the decoded key is `onfocus`.
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
    el.\u006fnfocus = h;
  }} />;
}
