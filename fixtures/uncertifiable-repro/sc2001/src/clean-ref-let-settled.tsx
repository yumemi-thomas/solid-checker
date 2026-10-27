// Clean. Mirrors viviana-ui-web apps/web/src/components/showcase/GlasselatedShell.tsx:18-28,39: a
// `let` assigned by a ref callback, handed to a module signal from onSettled (a leaf, where writes are legal).
import { createSignal, onSettled } from "solid-js";

const [shellRoot, setShellRoot] = createSignal<HTMLDivElement | undefined>(undefined);

export function Shell() {
  let root: HTMLDivElement | undefined;
  onSettled(() => {
    setShellRoot(root);
  });
  return (
    <div ref={(el) => (root = el)} data-has-root={shellRoot() ? "yes" : "no"}>
      shell
    </div>
  );
}
