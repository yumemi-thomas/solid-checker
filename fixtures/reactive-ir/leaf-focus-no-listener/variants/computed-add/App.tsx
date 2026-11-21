import { onCleanup, onSettled } from "solid-js";

// F3 of the review: `el[add]("focus", h)` with `const add = "addEventListener"` is the registration its plain spelling is.
const h = () => {
  onCleanup(() => {});
};
const add = "addEventListener" as const;
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    el[add]("focus", h);
  }} />;
}
