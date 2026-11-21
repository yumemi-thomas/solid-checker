import { onSettled } from "solid-js";
import { addEvent } from "@solidjs/web";
import { h } from "fz-handler";

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => { input = el; addEvent(el, "focus", h, false); }} />;
}
