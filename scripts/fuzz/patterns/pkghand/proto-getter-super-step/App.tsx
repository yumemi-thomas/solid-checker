// The check of #227 (G1): a getter that flushes is on a prototype literal and is read through `super` in an accessor of the derived object, in a step a package runs. No reference of `base` is written in the step.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const base = {
  get g(): number {
    flush();
    return 1;
  },
};
const o = {
  __proto__: base,
  get h(): number {
    void runStep(() => {
      void (super.g as any);
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
