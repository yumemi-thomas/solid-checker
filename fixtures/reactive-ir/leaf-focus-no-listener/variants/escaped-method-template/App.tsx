import { onCleanup, onSettled } from "solid-js";

// A template key under brackets, escaped: the cooked value is `addEventListener`.
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
    el[`\u0061ddEventListener`]("focus", h);
  }} />;
}
