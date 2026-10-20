import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const x = new XMLHttpRequest();
    x.addEventListener("readystatechange", register);
    x.open("GET", "/");
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
