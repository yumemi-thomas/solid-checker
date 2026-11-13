import { untrack } from "solid-js";
import kernel from "fz-obj";
function Wrap() {
  untrack(() => kernel).run(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
