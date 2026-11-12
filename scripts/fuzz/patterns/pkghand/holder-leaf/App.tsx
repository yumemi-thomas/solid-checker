import { flush } from "solid-js";
import { runLeafWith } from "fz-step";
const opts = {
  run: () => {
    flush();
  },
};
export function App() {
  setTimeout(() => {
    runLeafWith(opts);
  });
  return <p>x</p>;
}
