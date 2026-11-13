import { createRoot } from "solid-js";
import { runStep } from "fz-step";
setTimeout(() => {
  createRoot(() => {
    void runStep(() => {});
  });
});
export function App() {
  return <p>x</p>;
}
