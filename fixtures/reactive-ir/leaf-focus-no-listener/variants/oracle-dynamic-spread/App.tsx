import { onSettled } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { h } from "fz-handler";

const handlers = Object.fromEntries([["onFocus", h]]);

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <Dynamic component="input" {...handlers} ref={(el: HTMLInputElement) => { input = el; }} />;
}
