import { onCleanup, onSettled } from "solid-js";
import { template } from "@solidjs/web";

// F6 (inferred, as `runtime-set-attribute`): `template(html)` parses markup with `innerHTML`, and an inline handler in it runs on the stack of `focus()`.
const h = () => {
  onCleanup(() => {});
};
(globalThis as { hook?: () => void }).hook = h;
const make = template('<input onfocus="try { hook() } catch (e) {}">');

export function Leaf() {
  const node = make() as HTMLInputElement;
  onSettled(() => {
    node.focus();
  });
  return <div ref={(el) => { el.append(node); }} />;
}
