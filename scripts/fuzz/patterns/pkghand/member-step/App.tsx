import { flush } from "solid-js";
import { runStepWith } from "fz-step";
export function App() {
  setTimeout(() => {
    runStepWith({
      run: () => {
        flush();
      },
    });
  });
  return <p>x</p>;
}
