// Negative: the same early call from a createRoot callback.
import { createRoot, createSignal } from "solid-js";

const [, setCount] = createSignal(0);
createRoot(() => {
  eval("App()");
});
const late = () => {
  setCount(1);
};
export function App() {
  late();
  return <button />;
}
