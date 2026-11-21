import { onCleanup, onSettled } from "solid-js";

// An escaped `addEventListener` on the document, for the event that bubbles (`focusin`).
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
    document.\u0061ddEventListener("focusin", h);
  }} />;
}
