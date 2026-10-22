// Negative: Vite turns this eager glob into static imports of `glob-b.tsx`,
// which imports this module back: a cycle no import statement shows.
import { createSignal } from "solid-js";
const modules = import.meta.glob("./glob-b.tsx", { eager: true });
void modules;
const [, setValue] = createSignal(0);
export const helper = () => {
  setValue(1);
};
