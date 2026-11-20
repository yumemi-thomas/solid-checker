import { onCleanup, onSettled } from "solid-js";

// The same markup with the attribute name written by a unicode escape in the string (`o\u006efocus`): the cooked text is read, not the source.
const h = () => {
  onCleanup(() => {});
};
(globalThis as { hook?: () => void }).hook = h;

export function Leaf() {
  let box: HTMLDivElement | undefined;
  onSettled(() => {
    (box?.firstElementChild as HTMLInputElement | null | undefined)?.focus();
  });
  return <div ref={(el) => { box = el; }} innerHTML={'<input o\u006efocus="try { hook() } catch (e) {}">'} />;
}
