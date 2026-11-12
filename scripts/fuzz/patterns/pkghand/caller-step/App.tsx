import { flush } from "solid-js";
import { runStep } from "fz-step";
function helper() {
  flush();
}
export function App() {
  setTimeout(() => {
    runStep(() => helper());
  });
  return <p>x</p>;
}
