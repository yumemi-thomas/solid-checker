// Corpus adjudication 2026-10-23 (scripts/corpus/reviewed-findings.json):
// viviana-ui-web Header.tsx:26. onCleanup inside an owner-backed onSettled
// callback; rc.17 throws CLEANUP_IN_FORBIDDEN_SCOPE there in dev
// (@solidjs/signals dist/dev.js:2390-2402).
import { createSignal, onCleanup, onSettled } from "solid-js";

function useScrollDirection() {
  const [visible, setVisible] = createSignal(true);
  onSettled(() => {
    const handle = () => setVisible(window.scrollY < 50);
    window.addEventListener("scroll", handle, { passive: true });
    onCleanup(() => window.removeEventListener("scroll", handle));
  });
  return visible;
}

export function Header() {
  const visible = useScrollDirection();
  return <header>{String(visible())}</header>;
}
