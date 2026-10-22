import { createSignal } from "solid-js";

// The pinned rc.17 compiler refuses this file: `attr:` on an intrinsic
// element. The same untracked read as App.tsx is uncertifiable here, and the
// file gets the SC9015 notice.
export function Linked() {
  const [count] = createSignal(0);
  const seen = count();
  return <p attr:data-z={seen}>{seen}</p>;
}
