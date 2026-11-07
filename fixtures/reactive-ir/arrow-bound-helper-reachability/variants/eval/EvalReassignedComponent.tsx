// Negative: a direct `eval` can rebind the exported component, so the
// function the module-level createRoot runs may not be the one written here.
// A root is a root only while its binding is never written.
import { createRoot, createSignal } from "solid-js";

const [, setCount] = createSignal(0);
export function App() {
  setCount(1);
  return <button />;
}
eval("App = () => null");
createRoot(() => App());

function Bad() {
  setCount(2);
  return <button />;
}
eval("Bad = () => null");
export function Host() {
  return <Bad />;
}
