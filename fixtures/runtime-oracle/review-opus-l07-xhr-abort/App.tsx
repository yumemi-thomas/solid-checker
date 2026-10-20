import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const x = new XMLHttpRequest();
    x.open("GET", "/");
    x.send();
    x.addEventListener("abort", register);
    x.abort();
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
