// The re-review of #222, N1: an element created under `typeof` in the package component's children.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function Own() {
  flush();
  return null;
}
setTimeout(() => {
  document.body.append((<StepView>{typeof (<Own />) as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
