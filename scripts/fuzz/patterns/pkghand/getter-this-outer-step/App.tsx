// The re-check of #226, N1: a method declared outside the literal, a member of it (`{ m }`), reads a getter through `this` in a step.
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
  m,
};
setTimeout(() => {
  o.m();
});

export function App() {
  return <p>x</p>;
}
