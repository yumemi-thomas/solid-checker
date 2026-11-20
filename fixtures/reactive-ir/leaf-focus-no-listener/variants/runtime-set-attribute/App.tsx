import { onCleanup, onSettled } from "solid-js";
import { setAttribute } from "@solidjs/web";

// F7 of the review (inferred: an inline handler's frame has no source URL, so the oracle cannot attribute it): `setAttribute(el, "onfocus", code)` writes an inline
// handler, which runs on the stack of `focus()`.
const h = () => {
  onCleanup(() => {});
};
(globalThis as { hook?: () => void }).hook = h;
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    setAttribute(el, "onfocus", "try { hook() } catch (e) {}");
  }} />;
}
