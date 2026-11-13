// The review of #222 check 3, R1: the same with an element child.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function Own() {
  flush();
  return null;
}
function Wrap(props: { children?: unknown }) {
  return <StepView render={() => {
    void props.children;
  }} />;
}
setTimeout(() => {
  document.body.append((<Wrap><Own /></Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
