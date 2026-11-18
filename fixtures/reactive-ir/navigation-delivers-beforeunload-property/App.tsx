import { createSignal } from "solid-js";

// ADR 1080 negative: `onbeforeunload` registers a listener by assignment.
window.onbeforeunload = () => {};
function navigate() {
  history.go(-1);
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
