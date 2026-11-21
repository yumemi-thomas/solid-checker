import { onCleanup, onSettled } from "solid-js";

// An edited input fires `change` when focus leaves it: a move delivers it (named, a page with no user edit does not show it).
export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return (
    <div>
      <input ref={(el) => (input = el)} />
      <input onChange={() => onCleanup(() => {})} />
    </div>
  );
}
