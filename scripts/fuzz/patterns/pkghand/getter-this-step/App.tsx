// The review of #226, B3: an object literal's getter read through `this` in its own method's closure, run in a step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
const o = {
  get g(): number {
    flush();
    return 1;
  },
  m() {
    void runStep(() => {
      void this.g;
    });
  },
};
setTimeout(() => {
  o.m();
});

export function App() {
  return <p>x</p>;
}
