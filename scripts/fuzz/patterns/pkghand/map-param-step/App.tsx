// The review of #217, A: a project function whose parameter goes into `[g].map(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function via(g: () => void) {
  const rs = [g].map((f) => runStep(f));
}
setTimeout(() => {
  via(cb);
});

export function App() {
  return <p>x</p>;
}
