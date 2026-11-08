// Clean. Mirrors helge-dev src/components/NavBar.tsx:25-32: an effect's apply returns its teardown,
// where onCleanup in it has no owner (cleanup-in-effect-apply).
import { createEffect, createSignal } from "solid-js";

export function Nav() {
  const [opened, setOpened] = createSignal(false);
  createEffect(opened, (isOpen) => {
    if (!isOpen) return;
    const close = () => setOpened(false);
    document.addEventListener("click", close);
    return () => document.removeEventListener("click", close);
  });
  return <button onClick={() => setOpened(true)}>menu</button>;
}
