// The review of #217, A: `[[cb]].flat().map(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  const rs = [[cb]].flat().map((f) => runStep(f));
});

export function App() {
  return <p>x</p>;
}
