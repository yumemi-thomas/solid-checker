import { onCleanup } from "solid-js";

// `document`'s event path is `[document, window]`: no element is on it.
export function DocumentDispatch() {
  document.dispatchEvent(new Event("visibilitychange"));
  return <button onClick={() => { onCleanup(() => undefined); }}>unrelated</button>;
}
