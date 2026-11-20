import { onCleanup, onSettled } from "solid-js";
import { setProperty } from "@solidjs/web";

// `setProperty(node, name, value)` is `node[name] = value`: a handler property under a name the caller gives.
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
    setProperty(el, "onfocus", h);
  }} />;
}
