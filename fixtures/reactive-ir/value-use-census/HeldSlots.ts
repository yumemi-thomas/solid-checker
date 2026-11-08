import { createSignal } from "solid-js";

// Each holder keeps the source where no row reads it: the obligation is at
// every use, in this file and in each file that imports it.
export const [iifeRead, iifeWrite] = (() => {
  const [r, setR] = createSignal(1);
  return [r, setR] as const;
})();

const [a, b] = createSignal(1);
export let letRead = a;
export let assigned: () => number;
assigned = a;

class Box {
  r = a;
  setR = b;
}
export const box = new Box();
