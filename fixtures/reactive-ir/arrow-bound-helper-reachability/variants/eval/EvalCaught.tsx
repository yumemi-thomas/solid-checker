// Negative: a direct `eval` calls `App` before `late` is initialized.
import { createSignal } from "solid-js";

const [, setCount] = createSignal(0);
try {
  eval("App()");
} catch {}
const late = () => {
  setCount(1);
};
function App() {
  late();
  return <button />;
}
