import { onCleanup, onSettled } from "solid-js";

// ADR 1151 round 1: an identifier written with a unicode escape names the same member (`addEventListener`): compared by its decoded name.
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
    el.\u0061ddEventListener("focus", h);
  }} />;
}
