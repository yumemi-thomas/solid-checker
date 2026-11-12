import { flush } from "solid-js";
import { runStep } from "fz-step";
function wrap(run: () => void) {
  runStep(run);
}
export function App() {
  setTimeout(() => {
    wrap(() => {
      flush();
    });
  });
  return <p>x</p>;
}
