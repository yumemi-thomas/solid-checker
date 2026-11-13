// The check of #227 (G1, the method neighbour): a method that flushes is on a prototype literal and is called through `this` in a step a package runs.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  m(): void {
    flush();
  },
};
const o = {
  __proto__: base,
  get h(): number {
    void runStep(() => {
      (this as any).m();
    });
    return 1;
  },
};
setTimeout(() => {
  void o.h;
});

export function App() {
  return <p>x</p>;
}
