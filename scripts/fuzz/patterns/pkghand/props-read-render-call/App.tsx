// The review of #222 check 3, R1: a project wrapper reads its children in a render callback it hands to an unread package's component.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function Wrap(props: { children?: unknown }) {
  return <StepView render={() => {
    void props.children;
  }} />;
}
setTimeout(() => {
  document.body.append((<Wrap>{helper()}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
