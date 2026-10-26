// Violation. Corpus: app-game packages/math-examples/camera-projection-svg/cube.tsx:50 and
// polygon.tsx:22: `attr:data-z` on an element (ADR 0399).
import { createSignal } from "solid-js";

export function Vertex() {
  const [z] = createSignal(1);
  return <line x1={0} y1={0} attr:data-z={z()} stroke="black" />;
}
