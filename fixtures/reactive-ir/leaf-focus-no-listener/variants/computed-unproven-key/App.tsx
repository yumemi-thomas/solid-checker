import { onCleanup, onSettled } from "solid-js";

// A key no fact proves (a parameter) may name `addEventListener`: a computed member called, written or kept is read as the registration it may be.
const h = () => {
  onCleanup(() => {});
};
function register(el: HTMLElement, method: "addEventListener") {
  el[method]("focus", h);
}

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    register(el, "addEventListener");
  }} />;
}
