// The re-review of #222, N1: an element created under `void` in the package component's children.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function Own() {
  flush();
  return null;
}
setTimeout(() => {
  document.body.append((<StepView>{void (<Own />)}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
