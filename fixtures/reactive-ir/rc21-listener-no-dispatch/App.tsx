import { createSignal } from "solid-js";

const [count] = createSignal(0);
const obj = { handle() { console.log(count()); } };
const shared = () => {
  console.log(count());
};
window.addEventListener("click", () => obj.handle());
window.addEventListener("click", shared);

export function App() {
  return <button onClick={shared} />;
}
