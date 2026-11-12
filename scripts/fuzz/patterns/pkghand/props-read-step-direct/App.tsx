// The review of #222 check 3, R1: the same with a flush written in the child expression.
import { flush } from "solid-js";
import { runStep, StepView } from "fz-step";
function Wrap(props: { children?: unknown }) {
  setTimeout(() => {
    runStep(() => {
      void props.children;
    });
  });
  return null;
}
setTimeout(() => {
  document.body.append((<Wrap>{(flush(), "x")}</Wrap>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
