import { createSignal } from "solid-js";

// ADR 1080 negative: `location.assign` to a fragment fires `popstate` before it returns (Chrome), and a listener for it is here.
window.addEventListener("popstate", () => {});
function navigate() {
  location.assign("#next");
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
