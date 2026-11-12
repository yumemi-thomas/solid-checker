// The review of #217, A, the review's N-B: the step the package runs calls the elements of the literal.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const rs = [cb].map((f) => f());
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
