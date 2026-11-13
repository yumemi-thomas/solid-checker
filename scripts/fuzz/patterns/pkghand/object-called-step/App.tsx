// The review of #217, A, the review's N-B: the step the package runs calls a member of an object literal.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const r = { run: cb }.run();
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
