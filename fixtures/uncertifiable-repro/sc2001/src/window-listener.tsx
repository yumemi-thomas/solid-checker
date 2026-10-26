// ai-memory-ui shell-search.tsx:28-35: a hook registers a keydown listener on
// `window` in its body; the listener writes. A function handed to lib.dom's
// addEventListener runs from event delivery, but nothing places it.
import { createSignal, onCleanup } from "solid-js";

export function useShellSearch() {
  const [open, setOpen] = createSignal(false);
  const onKey = (event: KeyboardEvent) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      setOpen(true);
    }
  };
  window.addEventListener("keydown", onKey);
  onCleanup(() => window.removeEventListener("keydown", onKey));
  return { open };
}

export function Shell() {
  const search = useShellSearch();
  return <p>{search.open() ? "Searching" : "Press Ctrl+K"}</p>;
}
