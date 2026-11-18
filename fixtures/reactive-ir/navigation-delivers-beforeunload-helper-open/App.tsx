import { createSignal } from "solid-js";

// ADR 1080 negative: one call of the listener helper passes a type nothing names.
function bindEvent(target: Window, type: string, handler: () => void) {
  target.addEventListener(type, handler);
}
bindEvent(window, "scroll", () => {});
bindEvent(window, String(Date.now()), () => {});
function navigate() {
  history.go(-1);
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
