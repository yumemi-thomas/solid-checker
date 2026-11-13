// The review of #217, A: `{ a: cb }.a.name.length > 0 && [cb].some(…)`.
import { flush } from "solid-js";
import { runStep } from "fz-step";
const cb = () => {
  flush();
};
setTimeout(() => {
  const ok =
    { a: cb }.a.name.length > 0 &&
    [cb].some((f) => {
      runStep(f);
      return true;
    });
});

export function App() {
  return <p>x</p>;
}
