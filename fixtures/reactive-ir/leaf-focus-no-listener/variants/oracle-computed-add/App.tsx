import { onSettled } from "solid-js";
import { h } from "fz-handler";

const add = "addEventListener" as const;

export function Leaf() {
  let input: HTMLInputElement | undefined;
  onSettled(() => {
    input?.focus();
  });
  return <input ref={(el) => { input = el; el[add]("focus", h); }} />;
}
