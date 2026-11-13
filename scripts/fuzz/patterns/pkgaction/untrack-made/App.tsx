import { untrack } from "solid-js";
import kernel from "fz-obj";
function Wrap(props: { k: typeof kernel }) {
  const run = untrack(() => props.k.run);
  run(() => {});
  return null;
}
export function App() {
  return <Wrap k={kernel} />;
}
