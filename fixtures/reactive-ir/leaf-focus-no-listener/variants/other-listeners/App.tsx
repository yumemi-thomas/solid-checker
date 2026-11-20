import { onCleanup, onSettled } from "solid-js";

// A `click` handler, a `keydown` listener and a `scroll` listener: none is delivered by a focus move, so the call is proved.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return (
    <input
      ref={(el) => {
        input = el;
        el.addEventListener("keydown", () => onCleanup(() => {}));
      }}
      onClick={() => onCleanup(() => {})}
    />
  );
}
