// The review of #222, E: a component written as a child of an unread component is created when that component reads its children (here inside an action).
import { flush } from "solid-js";
import { StepView } from "fz-step";
function Own(props: { children?: unknown }) {
  flush();
  return <div>{props.children as never}</div>;
}
setTimeout(() => {
  document.body.append((<StepView><Own /></StepView>) as unknown as Node);
});

export function App() {
  return <p>x</p>;
}
