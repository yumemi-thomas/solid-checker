import { onCleanup, onSettled } from "solid-js";
import { Dynamic } from "@solidjs/web";

// F4 of the review: `Dynamic` forwards the spread to the `<input>` it makes, and `Object.fromEntries` builds the key `onFocus` where no fact reads it.
const h = () => {
  onCleanup(() => {});
};
const handlers = Object.fromEntries([["onFocus", h]]);

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <Dynamic component="input" {...handlers} ref={(el: HTMLInputElement) => { input = el; }} />;
}
