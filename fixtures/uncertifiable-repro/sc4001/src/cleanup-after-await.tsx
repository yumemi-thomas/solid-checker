// Violation. Corpus: app-game webgpu-examples/rotating-cube-2/rotating-cube-2.tsx:303: onCleanup
// after an await runs from a microtask with no current owner.
import { onCleanup } from "solid-js";

async function main(canvas: HTMLCanvasElement) {
  const response = await fetch("/shader");
  const frame = requestAnimationFrame(() => canvas.focus());
  onCleanup(() => cancelAnimationFrame(frame));
  return response.ok;
}

export function Cube() {
  return <canvas ref={(canvas) => void main(canvas)} />;
}
