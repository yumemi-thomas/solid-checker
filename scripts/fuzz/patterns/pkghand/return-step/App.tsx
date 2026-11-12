import { flush } from "solid-js";
import { runStep } from "fz-step";
function make() {
  return () => {
    flush();
  };
}
export function App() {
  setTimeout(() => {
    runStep(make());
  });
  return <p>x</p>;
}
