import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const r = new FileReader();
    r.readAsText(new Blob(["x"]));
    r.addEventListener("abort", register);
    r.abort();
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
