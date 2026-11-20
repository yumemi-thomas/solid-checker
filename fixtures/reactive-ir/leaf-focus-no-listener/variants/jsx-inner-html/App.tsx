import { onCleanup, onSettled } from "solid-js";

// F5 of the review (inferred): a JSX `innerHTML` attribute is a markup write. The string holds an inline handler, which runs on the stack of `focus()` where the
// element it makes is focused.
const h = () => {
  onCleanup(() => {});
};
(globalThis as { hook?: () => void }).hook = h;

export function Leaf() {
  let box: HTMLDivElement | undefined;
  onSettled(() => {
    (box?.firstElementChild as HTMLInputElement | null | undefined)?.focus();
  });
  return <div ref={(el) => { box = el; }} innerHTML={'<input onfocus="try { hook() } catch (e) {}">'} />;
}
