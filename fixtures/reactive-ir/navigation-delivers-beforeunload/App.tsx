import { createSignal } from "solid-js";

// ADR 1080: `history.go` and `location.reload` deliver at most `beforeunload` before they return, `location.assign` at most
// `beforeunload` or `popstate` (a fragment-only URL). Nothing here listens for either, so the navigations deliver to no
// listener and withhold no handler's proof: the click handler's read and write stay certified.
function navigate() {
  history.go(-1);
  location.reload();
  location.assign("/next");
}

export function Page() {
  const [count, setCount] = createSignal(0);
  navigate();
  return <button onClick={() => setCount(count() + 1)}>{count()}</button>;
}
