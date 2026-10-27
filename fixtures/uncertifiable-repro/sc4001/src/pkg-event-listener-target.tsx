// app-game apps/abr-viewer/src/components/OptionMenu.tsx:50 (and 33 more calls of
// its kind): a package's `makeEventListener` calls `addEventListener` on its
// first parameter; `window` is a global whose runtime implementation the checker
// cannot select. It lives in this project, not sc9012: importing the package there
// makes the returned-callable case lose its result.
import { makeEventListener } from "@solid-primitives/event-listener";
import { createSignal } from "solid-js";

export function Menu(props: { onClose: () => void }) {
  const [open, setOpen] = createSignal(false);
  if (typeof window !== "undefined") {
    makeEventListener(window, "resize", () => props.onClose());
  }
  return <button onClick={() => setOpen(!open())}>{open() ? "close" : "open"}</button>;
}
