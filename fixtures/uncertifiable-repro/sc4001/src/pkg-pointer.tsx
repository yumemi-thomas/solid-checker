// app-game packages/paint/ui-example/components/cap.tsx:32: a ref callback calls a package primitive that creates an effect.
import { createPointerListeners } from "@solid-primitives/pointer";

export function Cap() {
  const pointerHover = (target: SVGPathElement) =>
    createPointerListeners({
      target,
      onOver: () => console.debug("over"),
      onOut: () => console.debug("out")
    });

  return <path ref={pointerHover} />;
}
