// The review of #217, A: `use([cb].map(…))`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
function use(value: unknown) {
  return value;
}
setTimeout(() => {
  use([cb].map((f) => runStep(f)));
});

export function App() {
  return <p>x</p>;
}
