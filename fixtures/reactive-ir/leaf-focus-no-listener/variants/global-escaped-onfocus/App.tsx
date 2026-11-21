import { onCleanup, onSettled } from "solid-js";

// The same global write with an escaped identifier: the decoded name is `onfocus`.
const h = () => {
  onCleanup(() => {});
};
\u006fnfocus = h;

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
