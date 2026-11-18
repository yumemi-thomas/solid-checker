import { createSignal } from "solid-js";

// ADR 1080 negative: a `beforeunload` listener may run on the navigation's stack, so the dispatch keeps withholding proofs.
window.addEventListener("beforeunload", () => {});
function navigate() {
  history.go(-1);
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
