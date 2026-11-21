import { onCleanup, onSettled } from "solid-js";
import { createFrame } from "@solidjs/web/frames/client";

// The frames tier fills the page from the server's markup (`template.innerHTML = html`) and wires listeners as it loads: audited as `Any`, so any import of it
// is a registration the scans cannot read.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    createFrame(el);
  }} />;
}
