// Negative: the same, with the glob written after `helper`.
import { createSignal } from "solid-js";
const [, setValue] = createSignal(0);
export const helper = () => {
  setValue(1);
};
export const plugins = import.meta.glob("./glob2-b.tsx", { eager: true });
