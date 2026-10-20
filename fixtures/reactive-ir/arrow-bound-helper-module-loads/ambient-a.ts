// Negative: `virtual:b` is an ambient module declaration (a bundler plugin
// supplies its code), which may load `ambient-b.tsx`.
import "virtual:b";
import { createSignal } from "solid-js";
const [, setValue] = createSignal(0);
export const helper = () => {
  setValue(1);
};
