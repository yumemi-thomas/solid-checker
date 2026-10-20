import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const x = new XMLHttpRequest();
    x.open("POST", "/");
    x.upload.addEventListener("loadstart", register);
    x.send("body");
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
