// The review of #217, D: an intrinsic element's `ref={cb}` is applied where the element is created, on the stack of the step.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const el = <div ref={cb} />;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
