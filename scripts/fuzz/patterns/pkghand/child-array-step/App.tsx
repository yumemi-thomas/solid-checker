// The review of #222, D: an array child of an intrinsic element: its function elements are called where the element is created.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function step() {
  const el = <p>{[cb] as never}</p>;
}
setTimeout(() => {
  runStep(step);
});

export function App() {
  return <p>x</p>;
}
