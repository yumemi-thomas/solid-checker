import { onCleanup, onSettled } from "solid-js";
import { mark } from "./mark";
function register() {
  onCleanup(() => {});
  mark("a");
}
export function Leaf() {
  onSettled(() => {
    const c = new AbortController();
    c.signal.onabort = register;
    c.abort();
  });
  return <div />;
}
export function App() { return <p id="ran-a">pending</p>; }
