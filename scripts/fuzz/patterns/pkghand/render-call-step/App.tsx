// The re-review of #222, N1: a call in a prop of the package's component runs when the component reads the prop.
import { flush } from "solid-js";
import { StepView } from "fz-step";
function make() {
  flush();
  return () => {};
}
setTimeout(() => {
  document.body.append((<StepView render={make()} />) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
