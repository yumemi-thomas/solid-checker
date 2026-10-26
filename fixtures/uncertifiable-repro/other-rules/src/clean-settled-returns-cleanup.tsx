// Clean. Mirrors spookysoftware-www domains/www/src/terminal.tsx:23-44: onSettled returns its
// teardown, where onCleanup inside it is SC3001 (cleanup-in-on-settled).
import { createSignal, onSettled } from "solid-js";

function useScrollDirection() {
  const [visible, setVisible] = createSignal(true);
  onSettled(() => {
    const handle = () => setVisible(window.scrollY < 50);
    window.addEventListener("scroll", handle);
    return () => window.removeEventListener("scroll", handle);
  });
  return visible;
}

export function Header() {
  const visible = useScrollDirection();
  return <header hidden={!visible()} />;
}
