import { onCleanup, onSettled } from "solid-js";
import { addEvent } from "@solidjs/web";

// F1 of the review of 35704e13e: `addEvent` is the runtime's own registrar (`node.addEventListener(name, handler)`), so an import of it that passes a focus
// event is a listener the scans of the program's own `addEventListener` never read.
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
    addEvent(el, "focus", h, false);
  }} />;
}
