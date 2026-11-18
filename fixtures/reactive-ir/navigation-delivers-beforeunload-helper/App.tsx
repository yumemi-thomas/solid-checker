import { createSignal } from "solid-js";

// ADR 1080: a listener helper's type parameter is followed to its calls, as @solidjs/router's `bindEvent`: every call
// passes a literal of another event, so nothing listens for `beforeunload`.
function bindEvent(target: Window, type: string, handler: () => void) {
  target.addEventListener(type, handler);
  return () => target.removeEventListener(type, handler);
}
bindEvent(window, "scroll", () => {});
bindEvent(window, "pagehide", () => {});
// `typeof addEventListener` reads no member: no registration (as @solidjs/router's `data/liveQuery.js`).
if (typeof addEventListener === "function") addEventListener("online", () => {}, { once: true });
function navigate() {
  history.go(-1);
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
