// The re-check of #226, N1: a method assigned to the object later (`o.m = m`) reads a getter through `this` in a step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function m(this: { g: number }) {
  void runStep(() => {
    void this.g;
  });
}
const o = {
  get g(): number {
    flush();
    return 1;
  },
  m() {},
};
o.m = m;
setTimeout(() => {
  o.m();
});

export function App() {
  return <p>x</p>;
}
