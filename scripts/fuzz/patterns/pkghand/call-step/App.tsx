import { flush } from "solid-js";
import { runStep } from "fz-step";
export function App() {
  setTimeout(() => {
    runStep.call(null, () => {
      flush();
    });
  });
  return <p>x</p>;
}
