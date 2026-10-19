import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const d = document.createElement("div");
    d.contentEditable = "true";
    document.body.append(d);
    d.addEventListener("input", register);
    const r = document.createRange(); r.selectNodeContents(d);
    const sel = getSelection()!; sel.removeAllRanges(); sel.addRange(r);
    document.execCommand("insertText", false, "x");
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
