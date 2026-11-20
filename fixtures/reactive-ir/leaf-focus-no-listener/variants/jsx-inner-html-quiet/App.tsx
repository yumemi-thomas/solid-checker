import { onCleanup, onSettled } from "solid-js";

// A markup-bearing attribute whose value is a constant with no event attribute and no script holds no code: the proof holds.
export function Leaf() {
  let box: HTMLDivElement | undefined;
  onSettled(() => {
    (box?.firstElementChild as HTMLElement | null | undefined)?.focus();
  });
  return <div ref={(el) => { box = el; }} innerHTML={'<b>bold</b>'} />;
}
