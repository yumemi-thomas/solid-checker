import { onCleanup, onSettled } from "solid-js";
import "fz-computed-listener";

// The package's source is read (a dependency source, as the project's), and its registration is by a computed key: `document["add" + "EventListener"](…)`.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => {
    input = el;
    void el;
  }} />;
}
