// The review of #217, A: the array literal read through a destructuring, then `[first].map(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
const list = [cb];
const [first] = list;
setTimeout(() => {
  const rs = [first].map((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
