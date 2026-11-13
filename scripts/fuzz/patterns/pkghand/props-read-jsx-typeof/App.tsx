// The review of #226, B2: a project wrapper reads its children in a JSX child expression of an unread package's component, which reads it in its step.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function helper() {
  flush();
  return "x";
}
function Wrap(props: { children?: unknown }) {
  return <StepView>{typeof props.children as never}</StepView>;
}
setTimeout(() => {
  document.body.append((<Wrap>{helper()}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
