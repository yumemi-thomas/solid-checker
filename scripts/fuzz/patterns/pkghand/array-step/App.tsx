import { flush } from "solid-js";
import { runStepAll } from "fz-step";
export function App() {
  setTimeout(() => {
    runStepAll([
      () => {
        flush();
      },
    ]);
  });
  return <p>x</p>;
}
