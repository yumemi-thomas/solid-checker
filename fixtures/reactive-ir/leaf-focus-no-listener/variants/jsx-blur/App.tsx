import { onCleanup, onSettled } from "solid-js";

// The first leaf callback focuses `a`; the second moves focus to `b`, which delivers `blur` to `a`'s handler on its own stack.
export function Leaf() {
  let a: HTMLInputElement | undefined;
  let b: HTMLInputElement | undefined;
  onSettled(() => {
    a?.focus();
  });
  onSettled(() => {
    b?.focus();
  });
  return (
    <div>
      <input ref={(el) => (a = el)} onBlur={() => onCleanup(() => {})} />
      <input ref={(el) => (b = el)} />
    </div>
  );
}
