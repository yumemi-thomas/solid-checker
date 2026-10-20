import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const d = document.createElement("dialog");
    document.body.append(d);
    d.showModal;
    d.addEventListener("cancel", register);
    d.open = true;
    d.requestClose();
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
