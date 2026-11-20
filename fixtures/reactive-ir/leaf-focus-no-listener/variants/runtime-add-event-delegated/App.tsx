import { onCleanup, onSettled } from "solid-js";
import { addEvent, delegateEvents } from "@solidjs/web";

// F2: the delegated form writes `$$focusin` on the element, which the document listener `delegateEvents` attached reads.
const h = () => {
  onCleanup(() => {});
};
delegateEvents(["focusin"]);
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    addEvent(el, "focusin", h, true);
  }} />;
}
