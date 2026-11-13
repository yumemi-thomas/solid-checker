import { createMemo } from "solid-js";
import kernel from "fz-obj";
function Wrap() {
  const get = createMemo(() => kernel);
  get().run(() => {});
  return null;
}
export function App() {
  return <Wrap />;
}
