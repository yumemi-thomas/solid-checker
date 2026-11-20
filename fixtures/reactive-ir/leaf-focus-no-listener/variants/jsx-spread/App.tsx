import { onCleanup, onSettled } from "solid-js";

// The spread's keys may name a handler: no fact says they do not (the object is empty on this run).
const attributes: Record<string, unknown> = JSON.parse("{}");

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} {...attributes} />;
}
