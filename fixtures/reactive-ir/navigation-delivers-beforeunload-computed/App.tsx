import { createSignal } from "solid-js";

// ADR 1080 negative: an event type nothing names may be `beforeunload`.
const kinds = ["resize", "beforeunload"];
window.addEventListener(kinds[Math.floor(Math.random() * 2)], () => {});
function navigate() {
  history.go(-1);
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
