import "fz-focus-listener";
import { onSettled } from "solid-js";

// An installed package registers a `focusin` listener where it loads (its source is read: a loaded package, not an unread one). Its handler
// runs on the stack of the leaf callback's `focus()`.
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => (input = el)} />;
}
