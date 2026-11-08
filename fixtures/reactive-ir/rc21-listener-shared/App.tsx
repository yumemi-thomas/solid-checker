import { createSignal, createStore } from "solid-js";

const [count] = createSignal(0);
const [store] = createStore({ v: 1 });
const shared = () => {
  console.log(count());
};
const stored = () => {
  console.log(JSON.stringify(store));
};
window.addEventListener("click", shared);
window.addEventListener("click", stored);

export function App() {
  window.dispatchEvent(new MouseEvent("click"));
  return <button onClick={shared} />;
}

export function Stored() {
  document.body.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  return <button onClick={stored} />;
}
