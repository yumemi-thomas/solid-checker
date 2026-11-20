import { onCleanup, onSettled } from "solid-js";

// An escaped event type that is no focus-family event (`keydown`): the cooked value is read, and a listener for it is none a focus move delivers: the proof holds.
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
    el.addEventListener("\u006beydown", h);
  }} />;
}
