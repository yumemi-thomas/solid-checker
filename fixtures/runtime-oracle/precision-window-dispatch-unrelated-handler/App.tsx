import { onCleanup } from "solid-js";
export function App() {
  // A listener on `window`, run by the dispatches below on the component body's stack: owned, no warning.
  window.addEventListener("oracle:pong", () => { onCleanup(() => undefined); });
  window.dispatchEvent(new CustomEvent("oracle:pong"));
  document.dispatchEvent(new Event("oracle:ping"));
  return <button id="oracle-unrelated" onClick={() => { onCleanup(() => undefined); }}>unrelated handler</button>;
}
