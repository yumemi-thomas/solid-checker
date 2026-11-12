// The re-review of #222, N1: an immediately called function in the package component's children.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function Own() {
  flush();
  return null;
}
setTimeout(() => {
  document.body.append((<StepView>{(() => <Own />)() as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
