import { onSettled } from "solid-js";
import { addEvent, delegateEvents } from "@solidjs/web";
import { h } from "fz-handler";

delegateEvents(["focusin"]);

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => { input = el; addEvent(el, "focusin", h, true); }} />;
}
