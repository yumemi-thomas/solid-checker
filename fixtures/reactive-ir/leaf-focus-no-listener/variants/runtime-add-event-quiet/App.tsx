import { onCleanup, onSettled } from "solid-js";
import { addEvent } from "@solidjs/web";

// The exemption: every call of the registrar passes a constant event type outside what a focus move delivers (`click`), so nothing here runs on the stack of
// `focus()`: the proof holds (no SC9012 at the call).
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
    addEvent(el, "click", h, false);
  }} />;
}
