import { createSignal } from "solid-js";

const [count, setCount] = createSignal(0);
const obj = { handle() { setCount(count() + 1); } };
window.addEventListener("click", () => obj.handle());
window.addEventListener("keydown", obj.handle.bind(obj));
window.addEventListener("resize", { handleEvent: obj.handle });

export function App() {
  window.dispatchEvent(new MouseEvent("click"));
  return <button />;
}
