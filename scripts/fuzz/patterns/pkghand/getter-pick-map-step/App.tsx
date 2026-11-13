// The re-check of #226, N2: one callback returns the flushing function or the getter object; the getter object is read in a step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
const flag = Math.random() > 2;
function helper(): number {
  flush();
  return 1;
}
const o = {
  get g(): number {
    return helper();
  },
};
const pick = (): any => (flag ? helper : o);
const got = [0].map(pick);
setTimeout(() => {
  void runStep(() => {
    void got[0].g;
  });
});

export function App() {
  return <p>x</p>;
}
