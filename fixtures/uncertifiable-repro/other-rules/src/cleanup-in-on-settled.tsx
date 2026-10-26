// Violation. Corpus: viviana-ui-web apps/web/src/components/Header.tsx:26, lib/glasselated.ts:48:
// onSettled in a hook the component calls owns what it returns; an onCleanup inside it throws in dev.
import { createSignal, onCleanup, onSettled } from "solid-js";

function useScrollDirection() {
  const [visible, setVisible] = createSignal(true);
  onSettled(() => {
    const handle = () => setVisible(window.scrollY < 50);
    window.addEventListener("scroll", handle);
    onCleanup(() => window.removeEventListener("scroll", handle));
  });
  return visible;
}

export function Header() {
  const visible = useScrollDirection();
  return <header hidden={!visible()} />;
}
