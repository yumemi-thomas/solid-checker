import { flush } from "solid-js";
import { runApply } from "fz-step";
const cb = () => {
  flush();
};
const again = cb;
export function App() {
  setTimeout(() => {
    runApply(again);
  });
  return <p>x</p>;
}
