// The re-review of #222, N1: an intrinsic element's ref in a callback of an inline call in the package component's children.
import { flush } from "solid-js";
import { StepView } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  document.body.append((<StepView>{[1].map(() => <div ref={cb} />) as never}</StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
