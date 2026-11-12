import { flush } from "solid-js";
import { runStep } from "fz-step";
function cb() {
  flush();
}
export function App() {
  setTimeout(() => {
    runStep(cb);
  });
  return <p>x</p>;
}
